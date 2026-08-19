// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Raw GPIO LED access for diagnostics before the normal driver stack is available.

#[cfg(feature = "bsp_rpi3")]
mod raw {
    use core::ptr::{read_volatile, write_volatile};

    /// Turn the activity LED on or off.
    ///
    /// # Safety
    ///
    /// Directly accesses GPIO hardware without coordinating with the GPIO driver.
    #[inline(never)]
    #[no_mangle]
    pub unsafe extern "C" fn _set_led(on: bool) {
        const GPIO_BASE: usize = 0x3f20_0000;
        const GPFSEL2: *mut u32 = (GPIO_BASE + 0x08) as *mut u32;
        const GPSET0: *mut u32 = (GPIO_BASE + 0x1c) as *mut u32;
        const GPCLR0: *mut u32 = (GPIO_BASE + 0x28) as *mut u32;
        const PIN_BIT: u32 = 1 << 29;

        let mut fsel = read_volatile(GPFSEL2);
        fsel &= !(7 << 27);
        fsel |= 1 << 27;
        write_volatile(GPFSEL2, fsel);

        // The activity LED signal is active-low.
        write_volatile(if on { GPCLR0 } else { GPSET0 }, PIN_BIT);
    }

    /// Delay for `count` arbitrary units.
    #[inline(never)]
    #[no_mangle]
    pub fn _delay_n(count: usize) {
        for _ in 0..count {
            for _ in 0..0x8_0000_u32 {
                core::hint::spin_loop();
            }
        }
    }
}

#[cfg(feature = "bsp_rpi4")]
mod raw {
    use core::ptr::{read_volatile, write_volatile};

    /// Turn the activity LED on or off.
    ///
    /// # Safety
    ///
    /// Directly accesses GPIO hardware without coordinating with the GPIO driver.
    #[inline(never)]
    #[no_mangle]
    pub unsafe extern "C" fn _set_led(on: bool) {
        const GPIO_BASE: usize = 0xfe20_0000;
        const GPFSEL4: *mut u32 = (GPIO_BASE + 0x10) as *mut u32;
        const GPSET1: *mut u32 = (GPIO_BASE + 0x20) as *mut u32;
        const GPCLR1: *mut u32 = (GPIO_BASE + 0x2c) as *mut u32;
        const PIN_BIT: u32 = 1 << (42 - 32);

        let mut fsel = read_volatile(GPFSEL4);
        fsel &= !(7 << 6);
        fsel |= 1 << 6;
        write_volatile(GPFSEL4, fsel);

        // The activity LED signal is active-low.
        write_volatile(if on { GPCLR1 } else { GPSET1 }, PIN_BIT);
    }

    /// Delay for `count` arbitrary units.
    #[inline(never)]
    #[no_mangle]
    pub fn _delay_n(count: usize) {
        for _ in 0..count {
            for _ in 0..0x10_0000_u32 {
                core::hint::spin_loop();
            }
        }
    }
}

use raw::{_delay_n, _set_led};

/// Blink `code` as binary Morse, using a dash for one and a dot for zero.
#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _blink_code(code: usize, word_delay: bool) {
    _set_led(false);
    if word_delay {
        _delay_n(7);
    }

    let highest_bit = (usize::BITS - code.leading_zeros()).saturating_sub(1);
    for bit in (0..=highest_bit).rev() {
        let on_units = if code & (1 << bit) == 0 { 1 } else { 3 };
        _set_led(true);
        _delay_n(on_units);
        _set_led(false);
        _delay_n(if bit == 0 { 3 } else { 1 });
    }
}
