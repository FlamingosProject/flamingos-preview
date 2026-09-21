// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2019-2023 Andre Richter <andre.richter@i4.com>

//! A synchronous page fault must reach the kernel panic path.

#![no_main]
#![no_std]

use libkernel::{bsp, cpu, exception, info, memory, test};

#[no_mangle]
unsafe fn kernel_init() -> ! {
    exception::handling_init();
    memory::init();
    bsp::driver::qemu_bring_up_console();

    info!("Causing a page fault by reading address 9 GiB");
    test::expect_panic();
    core::ptr::read_volatile((9_usize * 1024 * 1024 * 1024) as *const u64);

    cpu::qemu_exit_failure()
}
