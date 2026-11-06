//! Memory backing, inactive mappings, and initial CPU state for the first process.
//!
//! The lower 1 GiB uses a private level-2 root with 64 KiB pages. Existing
//! kernel and MMIO mappings remain in TTBR1 and will be accessible to the
//! eventual privileged process. Activation installs TTBR0 under an owning,
//! core-bound guard. A separate unsafe, nonreturning launch restores state and
//! transfers execution to the trusted image after code-cache synchronization.

#[cfg(target_arch = "aarch64")]
#[path = "_arch/aarch64/process.rs"]
mod arch_process;

pub use arch_process::InitialContext;

use crate::memory::{Address, Physical, Virtual, mmu};
use alloc::rc::Rc;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use program_image::{
    ProgramImage,
    pages::{PAGE_SIZE, PrepareError, PreparedImage, StackPages},
};

/// Exclusive end of the first process's lower virtual address space.
pub const ADDRESS_END: u64 = 1 << 30;

/// Number of backed 64 KiB stack pages (256 KiB total).
pub const STACK_PAGES: usize = 4;

/// Start of the unmapped guard page below the fixed stack reservation.
pub const STACK_GUARD_START: u64 = ADDRESS_END - ((STACK_PAGES + 1) * PAGE_SIZE) as u64;

/// Preparing process backing or its translation descriptors failed.
#[derive(Debug)]
pub enum PrepareProcessError {
    /// The ELF image cannot be represented by this page-backing policy.
    Image(PrepareError),
    /// Stack layout or allocation failed.
    Stack(PrepareError),
    /// Kernel address translation or table allocation/construction failed.
    Tables(&'static str),
}

/// The running kernel cannot safely activate this prepared process now.
#[derive(Debug, Eq, PartialEq)]
pub enum ActivationError {
    /// Activation is restricted to EL1.
    WrongExceptionLevel,
    /// Secondary cores are still parked; activation belongs to the boot core.
    WrongCore,
    /// The existing kernel MMU and caches must already be enabled.
    MmuNotReady,
    /// Another lower address space is already active.
    AlreadyActive,
    /// The root or current kernel translation attributes are incompatible.
    UnsupportedRegime,
}

/// Access permission to check with the processor's address-translation instruction.
#[derive(Clone, Copy)]
pub enum MemoryAccess {
    /// Check a privileged data read.
    Read,
    /// Check a privileged data write.
    Write,
}

/// Inspecting a live hardware mapping failed.
#[derive(Debug, Eq, PartialEq)]
pub enum MappingError {
    /// Hardware queries are restricted to the activation core.
    WrongCore,
    /// TTBR0 no longer identifies this process's active tables.
    NotActive,
    /// The processor rejected translation or permission; payload is raw PAR_EL1.
    TranslationFault(u64),
}

/// Scalars passed in x0–x3 to the trusted `init` entry.
pub struct InitArguments {
    /// Already-mapped, upper-address GPIO register base.
    pub gpio_base: Address<Virtual>,
    /// GPIO number driving the board's activity LED.
    pub pin: u8,
    /// Whether a low electrical output turns the LED on.
    pub active_low: bool,
    /// Trusted nonreturning kernel callback used only by the QEMU init fixture.
    #[cfg(feature = "test_build")]
    pub completion: usize,
}

/// Owned image/stack backing and private lower-address tables.
///
/// Prepared instances are inactive. Activation moves ownership into
/// [`ActiveProcess`], which disables the mappings before releasing this value.
/// The ordinary inspection methods below read table descriptors, not aliases.
pub struct ProcessMemory {
    tables: mmu::ProcessTranslationTable,
    image: PreparedImage,
    stack: StackPages,
}

impl ProcessMemory {
    /// Consume an ELF image, allocate image/stack pages, and build their mappings.
    ///
    /// Segments sharing a 64 KiB page are rejected. Prefix/suffix padding is
    /// zeroed, readable memory is required, and writable/executable flags are
    /// preserved for privileged execution. The stack uses four zeroed RW/XN
    /// pages at the top of the lower address space, with an unmapped page below.
    /// ELF segments must end before the guard. No hardware state changes.
    ///
    /// # Errors
    ///
    /// Returns [`PrepareProcessError`] if backing cannot be allocated, the image
    /// violates mapping policy, or a physical address/table cannot be prepared.
    pub fn prepare(image: ProgramImage) -> Result<Self, PrepareProcessError> {
        let image =
            PreparedImage::new(image, STACK_GUARD_START).map_err(PrepareProcessError::Image)?;
        let stack =
            StackPages::new(ADDRESS_END, STACK_PAGES).map_err(PrepareProcessError::Stack)?;
        let tables = mmu::ProcessTranslationTable::new(&image, &stack)
            .map_err(PrepareProcessError::Tables)?;
        Ok(Self {
            tables,
            image,
            stack,
        })
    }

    /// Entry address for the initial CPU context.
    pub fn entry(&self) -> u64 {
        self.image.entry()
    }

    /// Physical level-2 root address; returning it does not activate the tables.
    pub fn root_address(&self) -> Address<Physical> {
        self.tables.root()
    }

    /// Number of privately backed 64 KiB image and stack pages, excluding tables.
    pub fn page_count(&self) -> usize {
        self.image
            .segments()
            .iter()
            .map(|segment| segment.pages().len())
            .sum::<usize>()
            + self.stack.pages().len()
    }

    /// Inclusive beginning of the backed stack; the preceding page is unmapped.
    pub fn stack_start(&self) -> u64 {
        self.stack.virtual_start()
    }

    /// Exclusive end of the stack and its initial, 16-byte-aligned SP.
    pub fn stack_top(&self) -> u64 {
        self.stack.top()
    }

    /// Inspect a prepared translation without dereferencing process memory.
    pub fn translate(&self, address: Address<Virtual>) -> Result<Address<Physical>, &'static str> {
        self.tables.translate(address)
    }

    /// Inspect a prepared page's privileged access and execute-never attributes.
    pub fn page_attributes(
        &self,
        page: mmu::PageAddress<Virtual>,
    ) -> Result<mmu::AttributeFields, &'static str> {
        self.tables.attributes(page)
    }
}

/// A prepared process owning its memory, stack, and initial register state.
///
/// [`Process::activate`] enables mappings without transferring execution.
/// [`ActiveProcess::run`] performs the separately reviewed, nonreturning launch.
/// Ownership keeps the context and all backing together through both phases.
pub struct Process {
    memory: ProcessMemory,
    context: InitialContext,
}

impl Process {
    /// Prepare a privileged, interrupt-masked process without activating it.
    ///
    /// # Errors
    ///
    /// Returns [`PrepareProcessError`] for image, stack, or table preparation
    /// failures. Allocations are released if preparation fails.
    pub fn prepare(image: ProgramImage) -> Result<Self, PrepareProcessError> {
        let memory = ProcessMemory::prepare(image)?;
        let context = InitialContext::new(memory.entry(), memory.stack_top());
        Ok(Self { memory, context })
    }

    /// Prepared memory and uninstalled translation descriptors.
    pub fn memory(&self) -> &ProcessMemory {
        &self.memory
    }

    /// Initial registers for a later transfer to the ELF entry point.
    pub fn context(&self) -> &InitialContext {
        &self.context
    }

    /// Enable this process's lower mappings while retaining kernel execution.
    ///
    /// The returned guard owns the process until it disables the mappings on
    /// drop. It cannot cross cores (`!Send`/`!Sync`). Kernel code, stack, IRQ
    /// handling, and MMIO remain in TTBR1; initial CPU state is not restored.
    /// [`ActiveProcess::run`] synchronizes copied instructions before executing them.
    ///
    /// # Errors
    ///
    /// Returns [`ActivationError`] if core, privilege, or current translation
    /// settings are unsuitable. On failure the prepared process is released and
    /// the original masks and translation configuration remain unchanged.
    pub fn activate(self) -> Result<ActiveProcess, ActivationError> {
        // Safety: preparation exclusively owns stable, private tables/backing.
        // The returned guard retains them and disables aliases before dropping.
        // Boot code is still on its high stack; other cores remain parked.
        // Need: install the live lower root under these ownership constraints.
        let previous = unsafe { arch_process::activate(self.memory.root_address())? };
        Ok(ActiveProcess {
            process: self,
            previous,
            core_affinity: PhantomData,
        })
    }
}

/// Owns active lower mappings while the kernel continues on its high stack.
///
/// Dropping this guard first disables lower walks, invalidates cached aliases,
/// and restores the prior TTBR0/TCR values, then releases the process backing.
/// Forgetting the guard retains its allocations rather than freeing live pages.
/// CPU affinity prevents safe transfer to a different core for destruction.
#[must_use = "retain this guard for as long as the process mappings must remain active"]
pub struct ActiveProcess {
    process: Process,
    previous: arch_process::TranslationState,
    core_affinity: PhantomData<Rc<()>>,
}

impl ActiveProcess {
    /// Prepared process metadata, still owned by this activation guard.
    pub fn process(&self) -> &Process {
        &self.process
    }

    /// Check the real hardware translation, including read/write permissions.
    ///
    /// Faults are returned in [`MappingError`] without a trapping data access.
    /// This also permits checking that upper kernel/MMIO mappings were retained.
    pub fn translate_hardware(
        &self,
        address: Address<Virtual>,
        access: MemoryAccess,
    ) -> Result<Address<Physical>, MappingError> {
        arch_process::translate(self.process.memory.root_address(), address, access)
    }

    /// Synchronize copied code and enter trusted `init`, retaining all backing.
    ///
    /// The old kernel stack and the consumed guard are intentionally retained:
    /// this first process has no exit, scheduler, or reclamation path. The entry
    /// receives GPIO base, pin, polarity, and zero (or a test completion callback).
    ///
    /// # Safety
    ///
    /// The loaded image must be trusted code implementing this scalar entry ABI
    /// and never returning. GPIO arguments must identify the board's valid mapped
    /// register block/pin. No other core or driver may concurrently reconfigure
    /// that pin. Caller must still be on the boot core's high kernel stack.
    ///
    /// # Need
    ///
    /// Executing privileged ELF code with direct MMIO access cannot be made safe
    /// by validating the ELF format; the caller must trust its behavior.
    pub unsafe fn run(mut self, arguments: InitArguments) -> ! {
        #[cfg(feature = "test_build")]
        let completion = arguments.completion as u64;
        #[cfg(not(feature = "test_build"))]
        let completion = 0;
        self.process.context.set_arguments([
            arguments.gpio_base.as_usize() as u64,
            u64::from(arguments.pin),
            u64::from(arguments.active_low),
            completion,
        ]);
        // Safety: the active owner retains unchanged code pages and their lower
        // aliases; no other core executes the image during synchronization.
        // Need: publish copied code to instruction fetch before its first execution.
        unsafe {
            arch_process::synchronize_code(&self.process.memory.image);
        }
        let retained = ManuallyDrop::new(self);
        // Safety: caller guarantees the trusted entry/ABI, and the retained owner
        // prevents teardown. The context's backed stack and code are now ready.
        // Need: restore the prepared state and leave the kernel stack via ERET.
        unsafe { arch_process::enter(&retained.process.context) }
    }

    #[cfg(feature = "test_build")]
    fn test_aliases(&mut self, expected_code: u8) {
        let entry = self.process.context.program_counter() + 3;
        let stack = self.process.context.stack_pointer() - 1;
        for (address, access) in [(entry, MemoryAccess::Read), (stack, MemoryAccess::Write)] {
            let address = Address::new(address as usize);
            assert_eq!(
                self.translate_hardware(address, access).unwrap(),
                self.process.memory.translate(address).unwrap()
            );
        }
        // Safety: &mut self excludes any live backing borrows. The checks above
        // establish both real translations and ownership; stack is RW normal DRAM.
        // Need: verify actual low-alias loads/stores without manufacturing references.
        let (code, written) = unsafe { arch_process::test_aliases(entry, stack) };
        assert_eq!(code, expected_code);
        assert_eq!(written, 0x5a);
        assert_eq!(
            self.process.memory.stack.pages().last().unwrap().bytes()[PAGE_SIZE - 1],
            0x5a
        );
    }
}

impl Drop for ActiveProcess {
    fn drop(&mut self) {
        // Safety: the guard cannot cross cores and still owns all backing. No
        // low references escape its API; execution/stack remain in TTBR1.
        // Need: remove live hardware aliases before Rust frees their allocations.
        unsafe {
            arch_process::deactivate(&self.previous);
        }
    }
}

/// Verify image/stack mappings and the initial register state in the QEMU boot test.
#[cfg(feature = "test_build")]
pub fn test_process_memory() {
    use alloc::vec;
    use program_image::pages::PAGE_SIZE;
    use program_image::{LoadSegment, Permissions};

    let source = ProgramImage {
        entry: 0x21_01a0,
        segments: vec![
            LoadSegment {
                virtual_address: 0x21_01a0,
                alignment: PAGE_SIZE as u64,
                permissions: Permissions {
                    read: true,
                    write: false,
                    execute: true,
                },
                data: vec![0, 0, 0, 0x14],
            },
            LoadSegment {
                // Cross the 512 MiB boundary to exercise both level-3 tables.
                virtual_address: 0x1fff_ff00,
                alignment: PAGE_SIZE as u64,
                permissions: Permissions {
                    read: true,
                    write: true,
                    execute: false,
                },
                data: vec![0; PAGE_SIZE],
            },
        ],
    };
    let process = Process::prepare(source).expect("Process preparation failed");
    let memory = process.memory();
    assert_eq!(memory.page_count(), 3 + STACK_PAGES);
    assert!(memory.root_address().is_page_aligned());
    for segment in memory.image.segments() {
        for (index, page) in segment.pages().iter().enumerate() {
            let address = Address::new(segment.virtual_start() as usize + index * PAGE_SIZE);
            let physical = memory.translate(address).unwrap();
            let backing = Address::new(page.bytes().as_ptr() as usize);
            assert_eq!(
                physical,
                mmu::try_kernel_virt_addr_to_phys_addr(backing).unwrap()
            );
            assert_eq!(memory.translate(address + 123).unwrap(), physical + 123);
            let attributes = memory
                .page_attributes(mmu::PageAddress::from(address))
                .unwrap();
            assert_eq!(attributes.execute_never, !segment.permissions().execute);
            assert_eq!(
                attributes.acc_perms,
                if segment.permissions().write {
                    mmu::AccessPermissions::ReadWrite
                } else {
                    mmu::AccessPermissions::ReadOnly
                }
            );
        }
    }
    assert!(memory.translate(Address::new(0)).is_err());
    assert!(
        memory
            .translate(Address::new(ADDRESS_END as usize))
            .is_err()
    );
    let entry = memory
        .translate(Address::new(memory.entry() as usize))
        .unwrap();
    assert_eq!(entry.offset_into_page(), 0x1a0);
    for (index, page) in memory.stack.pages().iter().enumerate() {
        let virtual_page = Address::new(memory.stack_start() as usize + index * PAGE_SIZE);
        let backing = Address::new(page.bytes().as_ptr() as usize);
        assert_eq!(
            memory.translate(virtual_page).unwrap(),
            mmu::try_kernel_virt_addr_to_phys_addr(backing).unwrap()
        );
        let attributes = memory
            .page_attributes(mmu::PageAddress::from(virtual_page))
            .unwrap();
        assert_eq!(attributes.mem_attributes, mmu::MemAttributes::CacheableDRAM);
        assert_eq!(attributes.acc_perms, mmu::AccessPermissions::ReadWrite);
        assert!(attributes.execute_never);
        assert!(page.bytes().iter().all(|&byte| byte == 0));
    }
    assert_eq!(memory.stack_start(), STACK_GUARD_START + PAGE_SIZE as u64);
    assert!(
        memory
            .translate(Address::new(STACK_GUARD_START as usize))
            .is_err()
    );
    assert!(
        memory
            .translate(Address::new(memory.stack_start() as usize - 1))
            .is_err()
    );
    let last_byte = Address::new(memory.stack_top() as usize - 1);
    assert!(memory.translate(last_byte).is_ok());
    assert_eq!(process.context().stack_pointer(), memory.stack_top());
    assert_eq!(process.context().stack_pointer() % 16, 0);
    assert_eq!(process.context().program_counter(), memory.entry());
    assert!(
        process
            .context()
            .registers()
            .iter()
            .all(|&register| register == 0)
    );
    // EL1h = 0b0101; DAIF bits [9:6] mask debug, SError, IRQ, and FIQ.
    assert_eq!(process.context().saved_status(), 0x3c5);
    let collision = ProgramImage {
        entry: STACK_GUARD_START,
        segments: vec![LoadSegment {
            virtual_address: STACK_GUARD_START,
            alignment: PAGE_SIZE as u64,
            permissions: Permissions {
                read: true,
                write: false,
                execute: true,
            },
            data: vec![0, 0, 0, 0x14],
        }],
    };
    assert!(matches!(
        Process::prepare(collision),
        Err(PrepareProcessError::Image(PrepareError::InvalidRange))
    ));
    crate::info!("Process test: image, guarded stack, and initial EL1h context verified");

    use aarch64_cpu::registers::{DAIF, TCR_EL1, TTBR0_EL1, TTBR1_EL1};
    use tock_registers::interfaces::Readable;
    let old_control = TCR_EL1.get();
    let old_root = TTBR0_EL1.get();
    let upper_root = TTBR1_EL1.get();
    let old_masks = DAIF.get();
    let expected_entry = memory
        .translate(Address::new(memory.entry() as usize))
        .unwrap();
    let kernel_address = Address::new(crate::version as *const () as usize);
    let expected_kernel = mmu::try_kernel_virt_addr_to_phys_addr(kernel_address).unwrap();
    let mmio_address = crate::bsp::memory::mmu::virt_mmio_remap_region().start_addr();
    let expected_mmio = mmu::try_kernel_virt_addr_to_phys_addr(mmio_address).unwrap();
    let replacement_image = |byte: u8| ProgramImage {
        entry: 0x21_01a0,
        segments: vec![LoadSegment {
            virtual_address: 0x21_01a0,
            alignment: PAGE_SIZE as u64,
            permissions: Permissions {
                read: true,
                write: false,
                execute: true,
            },
            data: vec![0, 0, 0, byte],
        }],
    };
    let mut active = process.activate().expect("Process activation failed");
    assert_eq!(TTBR1_EL1.get(), upper_root);
    assert_eq!(DAIF.get(), old_masks);
    assert_eq!(TCR_EL1.read(TCR_EL1::TG0), 1);
    assert_eq!(TCR_EL1.read(TCR_EL1::T0SZ), 34);
    assert_eq!(TCR_EL1.read(TCR_EL1::EPD0), 0);
    assert_eq!(
        TCR_EL1.get() & !((1 << 37) | 0xffbf),
        old_control & !((1 << 37) | 0xffbf)
    );
    let entry_address = Address::new(active.process().context().program_counter() as usize);
    assert_eq!(
        active
            .translate_hardware(entry_address, MemoryAccess::Read)
            .unwrap(),
        expected_entry
    );
    assert!(matches!(
        active.translate_hardware(entry_address, MemoryAccess::Write),
        Err(MappingError::TranslationFault(_))
    ));
    assert_eq!(
        active
            .translate_hardware(kernel_address, MemoryAccess::Read)
            .unwrap(),
        expected_kernel
    );
    assert_eq!(
        active
            .translate_hardware(mmio_address, MemoryAccess::Write)
            .unwrap(),
        expected_mmio
    );
    for address in [
        0,
        STACK_GUARD_START,
        active.process().memory().stack_start() - 1,
    ] {
        assert!(matches!(
            active.translate_hardware(Address::new(address as usize), MemoryAccess::Read),
            Err(MappingError::TranslationFault(_))
        ));
    }
    active.test_aliases(0x14);
    assert!(matches!(
        Process::prepare(replacement_image(0x42))
            .unwrap()
            .activate(),
        Err(ActivationError::AlreadyActive)
    ));
    // Allocate the replacement before releasing the first image so its physical
    // page must differ even though both images use the same virtual entry.
    let replacement = Process::prepare(replacement_image(0x6d)).unwrap();
    let replacement_entry = replacement.memory().translate(entry_address).unwrap();
    assert_ne!(replacement_entry, expected_entry);
    drop(active);
    assert_eq!(TCR_EL1.get(), old_control);
    assert_eq!(TTBR0_EL1.get(), old_root);
    assert_eq!(TTBR1_EL1.get(), upper_root);
    assert_eq!(DAIF.get(), old_masks);
    let mut active = replacement
        .activate()
        .expect("Replacement activation failed");
    assert_eq!(
        active
            .translate_hardware(entry_address, MemoryAccess::Read)
            .unwrap(),
        replacement_entry
    );
    active.test_aliases(0x6d);
    drop(active);
    assert_eq!(TCR_EL1.get(), old_control);
    assert_eq!(TTBR0_EL1.get(), old_root);
    assert_eq!(TTBR1_EL1.get(), upper_root);
    assert_eq!(DAIF.get(), old_masks);
    crate::info!(
        "Process activation test: translations, aliases, guard, root replacement, and teardown verified"
    );
}

/// Execute the compiled Rust init fixture and validate its GPIO writes in QEMU.
///
/// The fixture is built by `cargo xtask test-init`; its completion callback
/// terminates QEMU after checking the low process stack and EL1 state.
#[cfg(feature = "test_build")]
pub fn test_run_init() -> ! {
    use alloc::vec;
    let disk_image = include_bytes!(env!("FLAMINGOS_TEST_BOOT_FS"));
    let image =
        crate::storage::load_embedded_image(disk_image).expect("Invalid sparse init filesystem");
    let process = Process::prepare(image).expect("Init preparation failed");
    let active = process.activate().expect("Init activation failed");
    let mut registers = vec![0_u32; 64];
    let gpio_base = Address::new(registers.as_mut_ptr() as usize);
    // The nonreturning entry and completion hook retain exclusive access to
    // this normal-memory stand-in for GPIO until the QEMU process exits.
    core::mem::forget(registers);
    let arguments = InitArguments {
        gpio_base,
        pin: 29,
        active_low: true,
        completion: test_init_complete as *const () as usize,
    };
    crate::info!("Running compiled Rust init on its process stack");
    // Safety: the embedded fixture is built from the trusted init source with
    // qemu_test. Its GPIO stand-in is retained, and the completion ABI matches.
    // Need: exercise copied executable code and the actual EL1h context transfer.
    unsafe { active.run(arguments) }
}

/// Complete the nonreturning init fixture after its on/off cycle.
///
/// # Safety
///
/// `gpio` must be the retained 64-word GPIO stand-in supplied by test_run_init.
/// The trusted fixture calls this function with the original base address and
/// after completing both writes, with no concurrent accesses.
///
/// # Need
///
/// The callback reads volatile register-shaped memory initialized by another
/// executable and confirms the process used its prepared hardware context.
#[cfg(feature = "test_build")]
unsafe extern "C" fn test_init_complete(gpio: usize) -> ! {
    use aarch64_cpu::registers::{CurrentEL, DAIF, SP};
    use tock_registers::interfaces::Readable;
    let stack_pointer = SP.get();
    assert!(stack_pointer >= STACK_GUARD_START + PAGE_SIZE as u64);
    assert!(stack_pointer < ADDRESS_END);
    assert_eq!(stack_pointer % 16, 0);
    assert!(CurrentEL.matches_all(CurrentEL::EL::EL1));
    assert_eq!(DAIF.get() & 0x3c0, 0x3c0);
    // Safety: the caller contract identifies a live, retained 256-byte buffer;
    // all offsets are aligned and in bounds, and init has finished its writes.
    // Need: inspect the register writes without inventing MMIO references.
    let (function, set, clear) = unsafe {
        (
            core::ptr::read_volatile((gpio + 0x08) as *const u32),
            core::ptr::read_volatile((gpio + 0x1c) as *const u32),
            core::ptr::read_volatile((gpio + 0x28) as *const u32),
        )
    };
    assert_eq!((function >> 27) & 7, 1);
    assert_eq!(set, 1 << 29);
    assert_eq!(clear, 1 << 29);
    crate::info!(
        "Init execution verified: process stack, EL1h masks, GPIO output and on/off writes"
    );
    crate::cpu::qemu_exit_success()
}
