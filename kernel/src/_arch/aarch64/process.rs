//! AArch64 process context, translation activation, cache maintenance, and entry.

use super::{ActivationError, MappingError, MemoryAccess};
use crate::{
    bsp, cpu,
    memory::{Address, Physical, Virtual},
};
use aarch64_cpu::{asm::barrier, registers::*};
use core::arch::asm;
use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    registers::InMemoryRegister,
};

/// Register values for the first transfer to an ELF entry point.
///
/// EL1h selects SP_EL1. All asynchronous exceptions are initially masked;
/// unmasking requires a later interrupt/exception contract. General registers,
/// including the link register, start at zero; launch supplies scalar boot
/// arguments in x0–x3. No argc, argv, environment, or callable return address
/// is supplied by this entry convention.
///
/// This is owned metadata, not a live exception frame. The entry routine uses
/// its explicit C layout to restore registers before the nonreturning ERET.
#[derive(Debug)]
#[repr(C, align(16))]
pub struct InitialContext {
    registers: [u64; 31],
    stack_pointer: u64,
    program_counter: u64,
    saved_status: u64,
}

impl InitialContext {
    /// Construct state from the validated entry and owned stack's aligned top.
    pub(super) fn new(entry: u64, stack_top: u64) -> Self {
        let status = InMemoryRegister::<u64, SPSR_EL1::Register>::new(0);
        status.write(
            SPSR_EL1::M::EL1h
                + SPSR_EL1::D::Masked
                + SPSR_EL1::A::Masked
                + SPSR_EL1::I::Masked
                + SPSR_EL1::F::Masked,
        );
        Self {
            registers: [0; 31],
            stack_pointer: stack_top,
            program_counter: entry,
            saved_status: status.get(),
        }
    }

    /// Initial x0 through x30, with x30 being the link register.
    pub fn registers(&self) -> &[u64; 31] {
        &self.registers
    }

    /// Initial SP_EL1, pointing just above the highest backed stack byte.
    pub fn stack_pointer(&self) -> u64 {
        self.stack_pointer
    }

    /// Initial PC, taken from the ELF image's validated entry address.
    pub fn program_counter(&self) -> u64 {
        self.program_counter
    }

    /// Initial SPSR_EL1 value restored by the exception-return-based launch.
    pub fn saved_status(&self) -> u64 {
        self.saved_status
    }

    /// Supply the four scalar boot arguments while leaving other registers zero.
    pub(super) fn set_arguments(&mut self, arguments: [u64; 4]) {
        self.registers[..4].copy_from_slice(&arguments);
    }
}

/// Make copied code visible to instruction fetch through its lower virtual alias.
///
/// # Safety
///
/// The image's lower mappings must be active on this core, with unchanged,
/// readable code backing. Other cores remain parked and no code is executing
/// from these pages while they are synchronized.
///
/// # Need
///
/// Data stores and instruction fetch are not automatically coherent on all
/// supported CPUs; cache maintenance must cover both virtual views.
pub(super) unsafe fn synchronize_code(image: &program_image::pages::PreparedImage) {
    let cache_type: u64;
    // Safety: this runs at EL1 with architectural cache registers accessible.
    // Need: the register crate does not expose CTR_EL0 and its cache-line sizes.
    unsafe {
        asm!("mrs {cache_type}, ctr_el0", cache_type = out(reg) cache_type, options(nomem, nostack, preserves_flags));
    }
    let data_line = 4usize << ((cache_type >> 16) & 0xf);
    let instruction_line = 4usize << (cache_type & 0xf);
    for segment in image
        .segments()
        .iter()
        .filter(|segment| segment.permissions().execute)
    {
        for page in segment.pages() {
            let start = page.bytes().as_ptr() as usize;
            for address in (start..start + program_image::pages::PAGE_SIZE).step_by(data_line) {
                // Safety: the address belongs to owned normal memory in the kernel map.
                // Need: clean copied instruction bytes to the point of unification.
                unsafe {
                    asm!("dc cvau, {address}", address = in(reg) address, options(nostack, preserves_flags));
                }
            }
        }
    }
    barrier::dsb(barrier::ISH);
    for segment in image
        .segments()
        .iter()
        .filter(|segment| segment.permissions().execute)
    {
        let start = segment.virtual_start() as usize;
        let end = start + segment.pages().len() * program_image::pages::PAGE_SIZE;
        for address in (start..end).step_by(instruction_line) {
            // Safety: active lower aliases cover this entire executable segment.
            // Need: invalidate the instruction-side view at the addresses to be executed.
            unsafe {
                asm!("ic ivau, {address}", address = in(reg) address, options(nostack, preserves_flags));
            }
        }
    }
    barrier::dsb(barrier::ISH);
    barrier::isb(barrier::SY);
}

/// Restore initial state and enter the executable without returning to Rust.
///
/// # Safety
///
/// The context must identify trusted executable code, an active backed stack,
/// and valid ABI arguments. Code caches must be synchronized. Backing and upper
/// kernel mappings must remain alive permanently. The entry must never return.
///
/// # Need
///
/// Switching SP, restoring all general registers, and ERET cannot be expressed
/// as a Rust call without retaining the old stack and calling convention.
pub(super) unsafe fn enter(context: &InitialContext) -> ! {
    // Safety: the caller owns valid mappings/context for a nonreturning launch.
    // Context remains readable on the high kernel stack after SP changes. x30
    // retains its address until the last load; ERET uses ELR rather than LR.
    // Need: perform the architectural transfer with no Rust code on the new stack.
    unsafe {
        asm!(
            "msr daifset, #15",
            "mov x30, {context}",
            "ldr x9, [x30, #{sp_offset}]",
            "ldr x10, [x30, #{pc_offset}]",
            "ldr x11, [x30, #{status_offset}]",
            "msr elr_el1, x10",
            "msr spsr_el1, x11",
            "mov sp, x9",
            "ldp x0, x1, [x30, #0]",
            "ldp x2, x3, [x30, #16]",
            "ldp x4, x5, [x30, #32]",
            "ldp x6, x7, [x30, #48]",
            "ldp x8, x9, [x30, #64]",
            "ldp x10, x11, [x30, #80]",
            "ldp x12, x13, [x30, #96]",
            "ldp x14, x15, [x30, #112]",
            "ldp x16, x17, [x30, #128]",
            "ldp x18, x19, [x30, #144]",
            "ldp x20, x21, [x30, #160]",
            "ldp x22, x23, [x30, #176]",
            "ldp x24, x25, [x30, #192]",
            "ldp x26, x27, [x30, #208]",
            "ldp x28, x29, [x30, #224]",
            "ldr x30, [x30, #240]",
            "eret",
            context = in(reg) context,
            sp_offset = const core::mem::offset_of!(InitialContext, stack_pointer),
            pc_offset = const core::mem::offset_of!(InitialContext, program_counter),
            status_offset = const core::mem::offset_of!(InitialContext, saved_status),
            options(noreturn),
        );
    }
}

/// Previous lower-translation configuration, owned by the core-bound active guard.
pub(super) struct TranslationState {
    control: u64,
    root: u64,
}

/// Restore the caller's exception masks on every return path.
struct ExceptionMask {
    saved: u64,
}

impl ExceptionMask {
    fn new() -> Self {
        let saved = DAIF.get();
        DAIF.set(saved | (0b1111 << 6));
        Self { saved }
    }
}

impl Drop for ExceptionMask {
    fn drop(&mut self) {
        DAIF.set(self.saved);
    }
}

/// Install a prepared lower root while retaining all upper-region configuration.
///
/// # Safety
///
/// `root` must describe immutable, valid 64 KiB tables for the lower 1 GiB.
/// Their backing and all mapped pages must remain owned and stable until
/// `deactivate` completes on this core. Execution and the current stack must
/// remain in the kernel's upper mappings. Other cores must remain parked.
///
/// # Need
///
/// Changing the live translation regime and issuing TLBI cannot express these
/// ownership and execution constraints through the register APIs alone.
pub(super) unsafe fn activate(
    root: Address<Physical>,
) -> Result<TranslationState, ActivationError> {
    if !CurrentEL.matches_all(CurrentEL::EL::EL1) {
        return Err(ActivationError::WrongExceptionLevel);
    }
    if cpu::smp::core_id::<u64>() != bsp::cpu::BOOT_CORE_ID {
        return Err(ActivationError::WrongCore);
    }
    let mask = ExceptionMask::new();
    if !SCTLR_EL1
        .matches_all(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable)
    {
        return Err(ActivationError::MmuNotReady);
    }
    if !TCR_EL1.matches_all(TCR_EL1::EPD0::DisableTTBR0Walks) {
        return Err(ActivationError::AlreadyActive);
    }
    if !root.is_page_aligned()
        || root.as_usize() >= (1 << 40)
        || !TCR_EL1.matches_all(
            TCR_EL1::TG1::KiB_64
                + TCR_EL1::EPD1::EnableTTBR1Walks
                + TCR_EL1::A1::TTBR1
                + TCR_EL1::SH1::Inner
                + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable,
        )
        || (MAIR_EL1.get() >> 8) & 0xff != 0xff
    {
        return Err(ActivationError::UnsupportedRegime);
    }
    let previous = TranslationState {
        control: TCR_EL1.get(),
        root: TTBR0_EL1.get(),
    };
    // Descriptor stores must be visible to the cacheable, inner-shareable walker.
    barrier::dsb(barrier::ISHST);
    TCR_EL1.modify(
        TCR_EL1::TG0::KiB_64
            + TCR_EL1::T0SZ.val(64 - super::ADDRESS_END.ilog2() as u64)
            + TCR_EL1::SH0::Inner
            + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::TBI0::Used
            + TCR_EL1::EPD0::DisableTTBR0Walks,
    );
    TTBR0_EL1.set_baddr(root.as_usize() as u64);
    barrier::isb(barrier::SY);
    // Safety: lower walks remain disabled and upper tables/ASID are unchanged.
    // Need: no register-crate API provides whole-regime invalidation, including globals.
    unsafe {
        asm!("tlbi vmalle1is", options(nostack, preserves_flags));
    }
    barrier::dsb(barrier::ISH);
    barrier::isb(barrier::SY);
    TCR_EL1.modify(TCR_EL1::EPD0::EnableTTBR0Walks);
    barrier::isb(barrier::SY);
    drop(mask);
    Ok(previous)
}

/// Disable lower aliases and restore the previous, walk-disabled configuration.
///
/// # Safety
///
/// Called on the activation core while executing on the high kernel stack.
/// The root and backing must still be alive, with no outstanding lower-alias
/// accesses. `previous` must belong to the current activation.
///
/// # Need
///
/// Aliases and cached translations must disappear before Rust releases their
/// owning allocations; ordinary object destruction cannot perform that operation.
pub(super) unsafe fn deactivate(previous: &TranslationState) {
    let mask = ExceptionMask::new();
    barrier::dsb(barrier::ISH);
    TCR_EL1.modify(TCR_EL1::EPD0::DisableTTBR0Walks);
    barrier::isb(barrier::SY);
    // Safety: walks are disabled; all accesses completed and backing is still owned.
    // Need: invalidate cached aliases before their physical pages can be reused.
    unsafe {
        asm!("tlbi vmalle1is", options(nostack, preserves_flags));
    }
    barrier::dsb(barrier::ISH);
    barrier::isb(barrier::SY);
    TTBR0_EL1.set(previous.root);
    TCR_EL1.set(previous.control);
    barrier::isb(barrier::SY);
    drop(mask);
}

/// Ask the hardware walker to check an address without performing a data access.
pub(super) fn translate(
    root: Address<Physical>,
    address: Address<Virtual>,
    access: MemoryAccess,
) -> Result<Address<Physical>, MappingError> {
    if cpu::smp::core_id::<u64>() != bsp::cpu::BOOT_CORE_ID {
        return Err(MappingError::WrongCore);
    }
    let mask = ExceptionMask::new();
    if !TCR_EL1.matches_all(TCR_EL1::EPD0::EnableTTBR0Walks)
        || TTBR0_EL1.get_baddr() != root.as_usize() as u64
    {
        return Err(MappingError::NotActive);
    }
    // Safety: the active guard owns valid tables; AT reports faults in PAR instead
    // of dereferencing the address. Masks prevent a handler from overwriting PAR.
    // Need: the register crate does not expose the AT instructions.
    unsafe {
        match access {
            MemoryAccess::Read => {
                asm!("at s1e1r, {address}", address = in(reg) address.as_usize(), options(nostack, preserves_flags))
            }
            MemoryAccess::Write => {
                asm!("at s1e1w, {address}", address = in(reg) address.as_usize(), options(nostack, preserves_flags))
            }
        }
    }
    barrier::isb(barrier::SY);
    let result = PAR_EL1.get();
    drop(mask);
    if result & 1 != 0 {
        return Err(MappingError::TranslationFault(result));
    }
    Ok(Address::new(
        (((result >> 12) & ((1 << 36) - 1)) << 12) as usize | (address.as_usize() & 0xfff),
    ))
}

/// Exercise two validated, active aliases without creating Rust references to them.
///
/// # Safety
///
/// Caller must exclusively borrow the active owner; `entry` must be readable
/// backed image data and `stack` a writable backed stack byte on this core.
/// No Rust references to the backing bytes may remain live across the store.
///
/// # Need
///
/// Assembly tests actual translated loads/stores without treating a second
/// virtual alias as a separately borrowed Rust allocation.
#[cfg(feature = "test_build")]
pub(super) unsafe fn test_aliases(entry: u64, stack: u64) -> (u8, u8) {
    let code: u32;
    let written: u32;
    // Safety: the caller validates ownership, hardware translation, and permissions.
    // Need: exercise the lower aliases themselves, beyond software table inspection.
    unsafe {
        asm!("ldrb {code:w}, [{entry}]", "strb {value:w}, [{stack}]", "ldrb {written:w}, [{stack}]",
            code = out(reg) code, written = out(reg) written,
            entry = in(reg) entry, stack = in(reg) stack, value = in(reg) 0x5a_u32,
            options(nostack, preserves_flags));
    }
    (code as u8, written as u8)
}
