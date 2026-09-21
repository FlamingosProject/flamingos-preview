// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2020-2023 Andre Richter <andre.o.richter@gmail.com>

//! IRQ handling sanity tests.

#![no_main]
#![no_std]

use libkernel::{bsp, cpu, exception, memory};

#[no_mangle]
unsafe fn kernel_init() -> ! {
    memory::init();
    bsp::driver::qemu_bring_up_console();

    exception::handling_init();
    exception::asynchronous::local_irq_unmask();

    local_irq_mask_works();
    local_irq_unmask_works();
    local_irq_mask_save_works();

    cpu::qemu_exit_success()
}

/// Check that IRQ masking works.
fn local_irq_mask_works() {
    // Precondition: IRQs are unmasked.
    assert!(!exception::asynchronous::is_local_irq_masked());

    exception::asynchronous::local_irq_mask();
    assert!(exception::asynchronous::is_local_irq_masked());

    // Restore earlier state.
    exception::asynchronous::local_irq_unmask();
}

/// Check that IRQ unmasking works.
fn local_irq_unmask_works() {
    // Precondition: IRQs are masked.
    exception::asynchronous::local_irq_mask();
    assert!(exception::asynchronous::is_local_irq_masked());

    exception::asynchronous::local_irq_unmask();
    assert!(!exception::asynchronous::is_local_irq_masked());
}

/// Check that IRQ mask save is saving "something".
fn local_irq_mask_save_works() {
    // Precondition: IRQs are unmasked.
    assert!(!exception::asynchronous::is_local_irq_masked());

    let first = exception::asynchronous::local_irq_mask_save();
    assert!(exception::asynchronous::is_local_irq_masked());

    let second = exception::asynchronous::local_irq_mask_save();
    assert_ne!(first, second);

    exception::asynchronous::local_irq_restore(first);
    assert!(!exception::asynchronous::is_local_irq_masked());
}
