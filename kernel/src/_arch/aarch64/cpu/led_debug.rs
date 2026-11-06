// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Raw GPIO LED access for diagnostics before the normal driver stack is available.

#[cfg(feature = "bsp_rpi3")]
mod raw {
    use super::super::led::set_led;

    /// Turn the activity LED on or off before address translation is enabled.
    ///
    /// # Safety
    ///
    /// The board must have direct ACT wiring on GPIO29. The physical GPIO
    /// registers must be accessible, with no concurrent GPIO access.
    ///
    /// # Need
    ///
    /// Boot diagnostics run before the GPIO driver and virtual mapping exist.
    // Safety: This unique symbol is called with the documented C ABI.
    // Need: Early assembly and debug tools locate the unmangled entry.
    #[inline(never)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn _set_led(on: bool) {
        // Safety: The caller guarantees the board wiring and physical mapping,
        // with exclusive access during early boot.
        // Need: Use the shared raw GPIO implementation before drivers exist.
        unsafe { set_led(0x3f20_0000, 29, true, on) };
    }

    /// Delay for `count` arbitrary units.
    #[inline(never)]
    #[unsafe(no_mangle)]
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
    use super::super::led::set_led;

    /// Turn the activity LED on or off before address translation is enabled.
    ///
    /// # Safety
    ///
    /// The board must have Pi 4 Model B active-high ACT wiring on GPIO42.
    /// The physical GPIO
    /// registers must be accessible, with no concurrent GPIO access.
    ///
    /// # Need
    ///
    /// Boot diagnostics run before the GPIO driver and virtual mapping exist.
    // Safety: This unique symbol is called with the documented C ABI.
    // Need: Early assembly and debug tools locate the unmangled entry.
    #[inline(never)]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn _set_led(on: bool) {
        // Safety: The caller guarantees the board wiring and physical mapping,
        // with exclusive access during early boot.
        // Need: Use the shared raw GPIO implementation before drivers exist.
        unsafe { set_led(0xfe20_0000, 42, false, on) };
    }

    /// Delay for `count` arbitrary units.
    #[inline(never)]
    #[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _blink_code(code: usize, word_delay: bool) {
    unsafe {
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
}
