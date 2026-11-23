// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

//! A simple sanity test to see if exception restore code works.

#![no_main]
#![no_std]

use libkernel::{bsp, cpu, exception, info, memory};

#[inline(never)]
fn nested_system_call() {
    #[cfg(target_arch = "aarch64")]
    {
        assert_eq!(
            flamingos_syscall::call::<0x1337>(0),
            flamingos_syscall::NOT_IMPLEMENTED,
        );
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        info!("Not supported yet");
        cpu::wait_forever();
    }
}

#[unsafe(no_mangle)]
unsafe fn kernel_init() -> ! {
    unsafe {
        exception::handling_init();
        memory::init();
        bsp::driver::qemu_bring_up_console();

        info!("Making a dummy system call");

        // Calling this inside a function indirectly tests if the link register is restored properly.
        nested_system_call();

        info!("Back from system call!");

        bsp::driver::test_led_syscalls();

        cpu::qemu_exit_success()
    }
}
