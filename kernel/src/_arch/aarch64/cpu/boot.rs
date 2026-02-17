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

use crate::{memory, memory::Address};
use aarch64_cpu::{asm, registers::*};
use fdt::Fdt;
use core::arch::global_asm;
use core::mem::MaybeUninit;
use tock_registers::interfaces::Writeable;

#[cfg(feature = "boot_trace")]
const BOOT_TRACE: u64 = 1;
#[cfg(not(feature = "boot_trace"))]
const BOOT_TRACE: u64 = 0;

pub const FDT_SIZE: usize = const std::mem::size_of::<Fdt>();


// Assembly counterpart to this file.
global_asm!(
    include_str!("boot.s"),
    CONST_CURRENTEL_EL2 = const 0x8,
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
#[inline(always)]
unsafe fn prepare_el2_to_el1_transition(
    virt_boot_core_stack_end_exclusive_addr: u64,
    virt_kernel_init_addr: u64,
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
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

pub static mut DEVICE_TREE: MaybeUninit<Fdt> = MaybeUninit::uninit();

// Safety: `device_tree` points at a valid dtb.
unsafe fn process_device_tree(device_tree: *const u8) {
    let fdt = Fdt::from_ptr(device_tree).unwrap();
    #[allow(static_mut_refs)]
    DEVICE_TREE.write(fdt);
}

// Safety: `DEVICE_TREE` must be initialized.
pub unsafe fn get_core_ids(core_ids: &mut [usize]) -> &[usize] {
    #[allow(static_mut_refs)]
    let dt = DEVICE_TREE.assume_init_ref();

    let num_cores = dt.cpus().count();
    assert!(num_cores <= core_ids.len());

    for (cid, cpu) in core_ids.iter_mut().zip(dt.cpus()) {
        assert!(cpu.ids().all().count() == 1);
        *cid = cpu.ids().first();
    }

    &core_ids[..num_cores]
}

#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _panic_code(n: usize) -> ! {
    loop {
        led_debug::_blink_code(n, true);
    }
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
pub unsafe extern "C" fn _start_rust(
    phys_kernel_tables_base_addr: u64,
    virt_boot_core_stack_end_exclusive_addr: u64,
    virt_kernel_init_addr: u64,
    device_tree: *const u8,
) -> ! {
    process_device_tree(device_tree);

    #[cfg(feature = "boot_trace")]
    led_debug::_blink_code(8, true);

    prepare_el2_to_el1_transition(
        virt_boot_core_stack_end_exclusive_addr,
        virt_kernel_init_addr,
    );

    // Turn on the MMU for EL1.
    let addr = Address::new(phys_kernel_tables_base_addr as usize);
    memory::mmu::enable_mmu_and_caching(addr).unwrap();

    // Use `eret` to "return" to EL1. Since virtual memory will already be enabled, this results in
    // execution of kernel_init() in EL1 from its _virtual address_.
    asm::eret()
}
