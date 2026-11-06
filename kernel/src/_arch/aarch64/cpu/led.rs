// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Raw LED output shared by boot diagnostics, the kernel, and init.

use core::ptr::{read_volatile, write_volatile};

/// Set the activity LED using the boot debugger's GPIO sequence.
///
/// Select output mode first, then write the pin's SET or CLR register.
/// `gpio_base` may be physical before the MMU is enabled or a mapped alias
/// afterward; this function does not change address spaces.
///
/// # Safety
///
/// `gpio_base` must be a live, aligned GPIO register mapping, with `pin` a
/// valid pin (0..=53) owned exclusively by this caller. `active_low` must
/// match the board wiring. No concurrent access may modify the pin's
/// function-select register. A retained, aligned register-shaped RAM buffer
/// may be supplied instead for tests.
///
/// # Need
///
/// Early boot and the standalone init executable have no GPIO driver;
/// hardware registers require volatile access through raw addresses.
#[inline(never)]
pub unsafe fn set_led(gpio_base: usize, pin: u8, active_low: bool, on: bool) {
    let pin = usize::from(pin);
    let fsel = (gpio_base + (pin / 10) * 4) as *mut u32;
    let shift = (pin % 10) * 3;
    let bit = 1u32 << (pin % 32);
    let bank_offset = (pin / 32) * 4;
    let set = (gpio_base + 0x1c + bank_offset) as *mut u32;
    let clear = (gpio_base + 0x28 + bank_offset) as *mut u32;

    // Safety: The caller guarantees live, aligned register addresses, a valid
    // exclusively owned pin, and no concurrent function-select updates.
    // Need: Configure and drive GPIO with the debugger's volatile MMIO sequence.
    unsafe {
        let value = read_volatile(fsel);
        write_volatile(fsel, (value & !(0b111 << shift)) | (0b001 << shift));
        write_volatile(if on == active_low { clear } else { set }, bit);
    }
}
