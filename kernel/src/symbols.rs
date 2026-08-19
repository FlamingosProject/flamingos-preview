// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Symbol lookup from the ELF retained by stage 1.
//!
//! Records refer to the kernel's linked virtual addresses. Names remain
//! borrowed from the ELF and are demangled only while being printed.

use crate::memory::{Address, Virtual};
pub use kernel_image::Symbol;
pub use rustc_demangle::demangle;

/// Retrieve a defined symbol containing a linked kernel virtual address.
///
/// Missing or malformed metadata produces no match, so diagnostic printing
/// can fall back to numeric addresses without allocating or panicking.
pub fn lookup_symbol(addr: Address<Virtual>) -> Option<Symbol<'static>> {
    #[cfg(feature = "chainloader")]
    {
        let _ = addr;
        None
    }
    #[cfg(not(feature = "chainloader"))]
    {
        let bytes = crate::cpu::boot::arch_boot::retained_kernel_elf()?;
        kernel_image::SymbolTable::parse(bytes)
            .ok()??
            .lookup(addr.as_usize() as u64)
    }
}
