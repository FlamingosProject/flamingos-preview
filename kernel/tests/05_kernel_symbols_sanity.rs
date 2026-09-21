// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Kernel symbol lookup sanity test.

#![no_main]
#![no_std]

use libkernel::{
    bsp, cpu,
    memory::{self, Address},
    symbols,
};

#[no_mangle]
unsafe fn kernel_init() -> ! {
    memory::init();
    bsp::driver::qemu_bring_up_console();

    let is_aligned = symbols::lookup_symbol(Address::new(
        libkernel::common::is_aligned as *const () as usize,
    ))
    .unwrap();
    assert!(is_aligned.name().ends_with("common::is_aligned"));

    let version =
        symbols::lookup_symbol(Address::new(libkernel::version as *const () as usize)).unwrap();
    assert!(version.name().ends_with("libkernel::version"));

    cpu::qemu_exit_success()
}
