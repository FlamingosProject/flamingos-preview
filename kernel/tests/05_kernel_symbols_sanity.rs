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

use core::fmt::{self, Write};

/// Capture diagnostic formatting without requiring Chapter 18's heap.
struct SymbolName {
    bytes: [u8; 512],
    len: usize,
}

impl Write for SymbolName {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn assert_name_suffix(raw: &str, suffix: &str) {
    let mut name = SymbolName {
        bytes: [0; 512],
        len: 0,
    };
    write!(name, "{:#}", symbols::demangle(raw)).unwrap();
    assert!(
        core::str::from_utf8(&name.bytes[..name.len])
            .unwrap()
            .ends_with(suffix)
    );
}

#[unsafe(no_mangle)]
unsafe fn kernel_init() -> ! {
    memory::init();
    bsp::driver::qemu_bring_up_console();

    let is_aligned = symbols::lookup_symbol(Address::new(
        libkernel::common::is_aligned as *const () as usize,
    ))
    .unwrap();
    assert_name_suffix(is_aligned.name(), "common::is_aligned");

    let version =
        symbols::lookup_symbol(Address::new(libkernel::version as *const () as usize)).unwrap();
    assert_name_suffix(version.name(), "libkernel::version");

    cpu::qemu_exit_success()
}
