// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2019-2023 Andre Richter <andre.richter@i4.com>

//! Timer sanity test kernel.

#![no_main]
#![no_std]

use core::time::Duration;
use libkernel::{bsp, cpu, exception, memory, time};

#[no_mangle]
unsafe fn kernel_init() -> ! {
    use memory::mmu::interface::MMU;

    exception::handling_init();
    memory::mmu::mmu()
        .enable_mmu_and_caching()
        .expect("MMU initialization failed");
    bsp::driver::qemu_bring_up_console();

    assert!(time::time_manager().uptime().as_nanos() > 0);

    let resolution = time::time_manager().resolution().as_nanos();
    assert!(resolution > 0);
    assert!(resolution < 100);

    let before = time::time_manager().uptime();
    time::time_manager().spin_for(Duration::from_secs(1));
    let after = time::time_manager().uptime();
    assert_eq!((after - before).as_secs(), 1);

    cpu::qemu_exit_success()
}
