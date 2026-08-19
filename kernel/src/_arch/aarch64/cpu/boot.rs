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

mod led_debug;

#[cfg(not(feature = "chainloader"))]
use crate::{memory, memory::Address};
#[cfg(not(feature = "chainloader"))]
use aarch64_cpu::{asm, registers::*};
use core::arch::global_asm;
#[cfg(not(feature = "chainloader"))]
use core::sync::atomic::AtomicBool;
#[cfg(not(feature = "chainloader"))]
use fdt::Fdt;
#[cfg(not(feature = "chainloader"))]
use tock_registers::interfaces::Writeable;

#[cfg(feature = "boot_trace")]
const BOOT_TRACE: u64 = 1;
#[cfg(not(feature = "boot_trace"))]
const BOOT_TRACE: u64 = 0;

// Normal and chainloader builds have deliberately different early-boot contracts.
#[cfg(not(feature = "chainloader"))]
global_asm!(
    include_str!("boot.s"),
    CONST_CURRENTEL_EL2 = const 0x8,
    CONST_CORE_ID_MASK = const 0b11,
    CONST_BOOT_TRACE = const BOOT_TRACE,
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
/// - The `bss` section is not initialized yet. The code must not use or reference it in any way.
/// - The HW state of EL1 must be prepared in a sound way.
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
#[no_mangle]
#[cfg(not(feature = "chainloader"))]
pub static mut BOOT_PARK: [AtomicBool; MAX_CORES] = [const { AtomicBool::new(false) }; MAX_CORES];

/// Consume the firmware device tree pointer retained across the MMU transition.
///
/// # Safety
///
/// - Must be called exactly once by the boot core after virtual memory is enabled.
/// - The firmware-provided pointer must identify a valid flattened device tree.
#[cfg(not(feature = "chainloader"))]
pub unsafe fn process_device_tree() {
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

/// Blink a fatal early-boot error code forever.
#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _panic_code(code: usize) -> ! {
    loop {
        led_debug::_blink_code(code, true);
    }
}

/// Rust entry point for a released secondary core.
///
/// # Safety
///
/// - The caller must provide the current core's firmware ID.
/// - The core must have a valid stack and must be released exactly once.
#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _start_core(_id: usize) -> ! {
    _panic_code(17)
}

/// The Rust entry of the `kernel` binary.
///
/// The function is called from the assembly `_start` function.
///
/// # Safety
///
/// - Exception return from EL2 must must continue execution in EL1 with `kernel_init()`.
#[inline(never)]
#[no_mangle]
#[cfg(not(feature = "chainloader"))]
pub unsafe extern "C" fn _start_rust(
    phys_kernel_tables_base_addr: u64,
    virt_boot_core_stack_end_exclusive_addr: u64,
    virt_kernel_init_addr: u64,
    device_tree: *const u8,
) -> ! {
    #[cfg(feature = "boot_trace")]
    led_debug::_blink_code(3, true);

    prepare_el2_to_el1_transition(
        virt_boot_core_stack_end_exclusive_addr,
        virt_kernel_init_addr,
        device_tree as u64,
    );

    // Turn on the MMU for EL1.
    let addr = Address::new(phys_kernel_tables_base_addr as usize);
    memory::mmu::enable_mmu_and_caching(addr).unwrap();

    // Use `eret` to "return" to EL1. Since virtual memory will already be enabled, this results in
    // execution of kernel_init() in EL1 from its _virtual address_.
    asm::eret()
}

/// Enter the relocated chainloader without changing exception level or architectural state.
#[no_mangle]
#[cfg(feature = "chainloader")]
pub unsafe extern "C" fn _start_rust(_device_tree: *const u8) -> ! {
    #[cfg(feature = "boot_trace")]
    led_debug::_blink_code(3, true);

    crate::chainloader::run()
}
