// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021-2023 Andre Richter <andre.o.richter@gmail.com>

//! Architectural boot code.
//!
//! # Orientation
//!
//! Since arch modules are imported into generic modules using the path attribute, the path of this
//! file is:
//!
//! crate::cpu::boot::arch_boot

pub mod led;
mod led_debug;

#[cfg(not(feature = "chainloader"))]
use crate::{
    bsp::memory::mmu::{self as bsp_mmu, BootMemoryLayout},
    memory,
};
#[cfg(not(feature = "chainloader"))]
use aarch64_cpu::{asm, registers::*};
use core::arch::global_asm;
#[cfg(not(feature = "chainloader"))]
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(not(feature = "chainloader"))]
use fdt::Fdt;
#[cfg(not(feature = "chainloader"))]
use tock_registers::interfaces::{Readable, Writeable};

#[cfg(feature = "boot_trace")]
const BOOT_TRACE: u64 = 1;
#[cfg(not(feature = "boot_trace"))]
const BOOT_TRACE: u64 = 0;

/// Maximum firmware device-tree size retained across the MMU transition.
#[cfg(not(feature = "chainloader"))]
const DEVICE_TREE_BUFFER_SIZE: usize = 256 * 1024;

#[cfg(not(feature = "chainloader"))]
#[repr(C, align(8))]
struct DeviceTreeBuffer([u8; DEVICE_TREE_BUFFER_SIZE]);

/// Firmware boot data copied before enabling the MMU.
///
/// # Safety
/// The symbol is unique and the boot core initializes its bounded backing
/// before any other code borrows it through the kernel's mapped address.
///
/// # Need
/// Early assembly needs a stable linker-visible destination for the DTB copy.
#[cfg(not(feature = "chainloader"))]
#[unsafe(no_mangle)]
static mut DEVICE_TREE_BUFFER: DeviceTreeBuffer = DeviceTreeBuffer([0; DEVICE_TREE_BUFFER_SIZE]);

/// Published by the boot core once the loaded ELF bytes have a read-only mapping.
///
/// # Safety
/// This symbol is unique in the kernel ELF and its BSS storage is naturally atomic-aligned.
///
/// # Need
/// The linker resolves its physical address for publication before the MMU is enabled.
#[cfg(not(feature = "chainloader"))]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.boot_elf_size")]
static BOOT_ELF_SIZE: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(feature = "chainloader"))]
// Safety: The linker script defines the base of the retained ELF mapping.
// Need: The boot image is loaded into reserved memory outside Rust statics.
unsafe extern "C" {
    static __boot_elf_virt_start: u8;
}

// Normal and chainloader builds have deliberately different early-boot contracts.
#[cfg(not(feature = "chainloader"))]
global_asm!(
    include_str!("boot.s"),
    CONST_CURRENTEL_EL2 = const 0x8,
    CONST_CORE_ID_MASK = const 0b11,
    CONST_BOOT_TRACE = const BOOT_TRACE,
    CONST_DEVICE_TREE_BUFFER_SIZE = const DEVICE_TREE_BUFFER_SIZE,
);

#[cfg(feature = "chainloader")]
global_asm!(
    include_str!("chainloader.s"),
    CONST_CORE_ID_MASK = const 0b11,
    CONST_BOOT_TRACE = const BOOT_TRACE,
);

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// Prepares the transition from EL2 to EL1.
///
/// # Safety
///
/// The given addresses must describe the mapped kernel entry, stack and device tree.
///
/// # Need
/// Set exception return state for the first transition into the kernel at EL1.
#[cfg(not(feature = "chainloader"))]
#[inline(always)]
unsafe fn prepare_el2_to_el1_transition(
    virt_boot_core_stack_end_exclusive_addr: u64,
    virt_kernel_init_addr: u64,
    virt_device_tree_addr: u64,
) {
    // Enable timer counter registers for EL1.
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);

    // No offset for reading the counters.
    CNTVOFF_EL2.set(0);

    // Set EL1 execution state to AArch64.
    HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);

    // Set up a simulated exception return.
    //
    // First, fake a saved program status where all interrupts were masked and SP_EL1 was used as a
    // stack pointer.
    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h,
    );

    // Second, let the link register point to kernel_init().
    ELR_EL2.set(virt_kernel_init_addr);

    // Set up SP_EL1 (stack pointer), which will be used by EL1 once we "return" to it. Since there
    // are no plans to ever return to EL2, just re-use the same stack.
    SP_EL1.set(virt_boot_core_stack_end_exclusive_addr);

    // Preserve the mapped device tree pointer across the exception return. kernel_init() consumes
    // and clears this scratch value before thread-local storage can use TPIDR_EL1.
    TPIDR_EL1.set(virt_device_tree_addr);
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Maximum number of CPU cores tracked by the early boot protocol.
pub const MAX_CORES: usize = 16;

/// CPU topology discovered from the firmware device tree.
#[derive(Debug)]
pub struct CoresInfo {
    /// Number of valid entries in [`Self::core_ids`].
    pub num_cores: usize,

    /// Firmware-provided CPU IDs.
    pub core_ids: [usize; MAX_CORES],
}

/// CPU topology populated once during early boot.
///
/// Access is restricted to the boot path while only the boot core is active.
pub static mut CORES_INFO: CoresInfo = CoresInfo {
    num_cores: 0,
    core_ids: [0; MAX_CORES],
};

/// Per-core release flags used by the assembly parking loop.
#[unsafe(no_mangle)]
#[cfg(not(feature = "chainloader"))]
pub static mut BOOT_PARK: [AtomicBool; MAX_CORES] = [const { AtomicBool::new(false) }; MAX_CORES];

/// The source ELF reserved by the loader for diagnostics throughout kernel execution.
#[cfg(not(feature = "chainloader"))]
pub(crate) fn retained_kernel_elf() -> Option<&'static [u8]> {
    if !CurrentEL.matches_all(CurrentEL::EL::EL1) || !SCTLR_EL1.matches_all(SCTLR_EL1::M::Enable) {
        return None;
    }
    let size = BOOT_ELF_SIZE.load(Ordering::Relaxed);
    if size == 0 {
        return None;
    }

    // Safety: Stage 1 reserved and initialized these bytes, and stage 2 mapped
    // that backing read-only for the lifetime of the kernel before publishing size.
    // Need: The retained bytes have a linker-defined address outside Rust statics.
    Some(unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(__boot_elf_virt_start), size) })
}

/// Consume the firmware device tree pointer retained across the MMU transition.
///
/// # Safety
///
/// - Must be called exactly once by the boot core after virtual memory is enabled.
/// - The firmware-provided pointer must identify a valid flattened device tree.
///
/// # Need
///
/// Early boot must read firmware-owned memory and populate once-only boot state.
#[cfg(not(feature = "chainloader"))]
pub unsafe fn process_device_tree() {
    // Safety: the boot contract supplies the preserved FDT and a single active core.
    // Need: access firmware memory and publish the existing raw boot-state globals.
    unsafe {
        let device_tree = TPIDR_EL1.get() as *const u8;
        TPIDR_EL1.set(0);

        // QEMU's raspi3b machine still supplies a legacy ATAG_CORE handoff instead of an FDT. Both
        // Raspberry Pi BSPs supported here have four cores, so retain the QEMU workflow explicitly.
        const ATAG_CORE: u32 = 0x5441_0001;
        const LEGACY_RPI_CORE_COUNT: usize = 4;
        let words = device_tree.cast::<u32>();
        if words.add(1).read_unaligned() == ATAG_CORE {
            let cores_info = core::ptr::addr_of_mut!(CORES_INFO);
            (*cores_info).num_cores = LEGACY_RPI_CORE_COUNT;
            for (id, slot) in (&mut (*cores_info).core_ids)[..LEGACY_RPI_CORE_COUNT]
                .iter_mut()
                .enumerate()
            {
                *slot = id;
            }
            return;
        }

        let fdt = Fdt::from_ptr(device_tree).unwrap();
        for compatible in fdt.root().compatible().all() {
            crate::bsp::record_act_led_board(compatible);
        }
        let num_cores = fdt.cpus().count();
        if num_cores > MAX_CORES {
            panic!();
        }
        let cores_info = core::ptr::addr_of_mut!(CORES_INFO);
        for (cid, cpu) in (*cores_info).core_ids.iter_mut().zip(fdt.cpus()) {
            if cpu.ids().all().count() != 1 {
                panic!();
            }
            *cid = cpu.ids().first();
        }
        CORES_INFO.num_cores = num_cores;
    }
}

/// Blink a fatal early-boot error code forever.
///
/// # Safety
///
/// Directly accesses GPIO hardware before the normal driver stack is available.
#[inline(never)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _panic_code(code: usize) -> ! {
    unsafe {
        loop {
            led_debug::_blink_code(code, true);
        }
    }
}

/// Rust entry point for a released secondary core.
///
/// # Safety
///
/// - The caller must provide the current core's firmware ID.
/// - The core must have a valid stack and must be released exactly once.
#[inline(never)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start_core(_id: usize) -> ! {
    unsafe { _panic_code(17) }
}

/// Build the kernel's translation tables and transition from physical EL2 to mapped EL1.
///
/// # Safety
/// `_start` must call this once on the boot core in EL2 with an initialized
/// physical stack and BSS. It must provide the physical table, layout and size
/// storage pointers, and the linked EL1 stack, entry point and device-tree addresses.
///
/// # Need
/// The bootstrap must initialize physical translation tables and perform an
/// exception return to enter the kernel using its linked virtual addresses.
#[inline(never)]
#[unsafe(no_mangle)]
#[cfg(not(feature = "chainloader"))]
pub(crate) unsafe extern "C" fn _start_rust(
    tables: *const (),
    virt_boot_core_stack_end_exclusive_addr: u64,
    virt_kernel_init_addr: u64,
    device_tree: *const u8,
    elf_size: usize,
    layout: *const BootMemoryLayout,
    retained_elf_size: *const AtomicUsize,
) -> ! {
    // Safety: Assembly provides the physical pointers, initialized stack and
    // BSS, and linked addresses corresponding to the supplied layout.
    // Need: Establish the CPU's initial virtual address space before entering EL1.
    unsafe {
        #[cfg(feature = "boot_trace")]
        led_debug::_blink_code(3, true);

        let Ok(root) = bsp_mmu::initialize_boot_tables(tables, &*layout, elf_size) else {
            _panic_code(0x14);
        };
        (*retained_elf_size).store(elf_size, Ordering::Relaxed);
        asm::barrier::dsb(asm::barrier::SY);

        prepare_el2_to_el1_transition(
            virt_boot_core_stack_end_exclusive_addr,
            virt_kernel_init_addr,
            device_tree as u64,
        );

        if memory::mmu::enable_mmu_and_caching(root).is_err() {
            _panic_code(0x15);
        }

        // Use `eret` to "return" to EL1. Since virtual memory will already be enabled, this results in
        // execution of kernel_init() in EL1 from its _virtual address_.
        asm::eret()
    }
}

/// Enter the relocated chainloader without changing exception level or architectural state.
///
/// # Safety
/// The loader assembly must have relocated the loader code, initialized its
/// stack and BSS, and preserved the firmware boot data.
///
/// # Need
/// This entry is called directly from assembly before an ordinary Rust runtime exists.
#[unsafe(no_mangle)]
#[cfg(feature = "chainloader")]
pub(crate) unsafe extern "C" fn _start_rust(_device_tree: *const u8) -> ! {
    #[cfg(feature = "boot_trace")]
    // Safety: The loader runs on the boot core with exclusive early access to GPIO.
    // Need: Boot tracing accesses physical GPIO before driver initialization.
    unsafe {
        led_debug::_blink_code(3, true);
    }

    crate::chainloader::run()
}
