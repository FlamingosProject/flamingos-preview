//! Raw LED manipulations (convenience functions for early
//! boot). These use raw pointer access and work from
//! anywhere. Thanks to Claude Code for the hard parts of
//! the implementation.

#[cfg(feature = "bsp_rpi3")]
mod raw {
    use core::ptr::{read_volatile, write_volatile};

    /// Turn the LED on or off.
    ///
    /// # Safety
    /// Directly accesses GPIO hardware
    #[inline(never)]
    #[no_mangle]
    pub unsafe extern "C" fn _set_led(on: bool) {
        const GPIO_BASE: usize = 0x3F20_0000;
        const GPFSEL2: *mut u32 = (GPIO_BASE + 0x08) as *mut u32;
        const GPSET0: *mut u32 = (GPIO_BASE + 0x1C) as *mut u32;
        const GPCLR0: *mut u32 = (GPIO_BASE + 0x28) as *mut u32;
        const PIN_BIT: u32 = 1 << 29;

        // Init: set GPIO 29 as output
        let mut fsel = read_volatile(GPFSEL2);
        fsel &= !(7 << 27);
        fsel |= 1 << 27;
        write_volatile(GPFSEL2, fsel);

        // XXX Sense is inverted
        if on {
            write_volatile(GPCLR0, PIN_BIT);
        } else {
            write_volatile(GPSET0, PIN_BIT);
        }
    }

    /// Delay for `n` "time counts" (arbitrary).
    #[inline(never)]
    #[no_mangle]
    pub extern "C" fn _delay_n(n: usize) {
        for _ in 0..n {
            for _ in 0..0x80000_u32 { core::hint::spin_loop(); }
        }
    }
}

#[cfg(feature = "bsp_rpi4")]
mod raw {
    use core::ptr::{read_volatile, write_volatile};

    /// Turn the LED on or off.
    ///
    /// # Safety
    /// Directly accesses GPIO hardware
    #[inline(never)]
    #[no_mangle]
    pub unsafe extern "C" fn _set_led(on: bool) {
        const GPIO_BASE: usize = 0xFE20_0000;
        const GPFSEL4: *mut u32 = (GPIO_BASE + 0x10) as *mut u32;
        const GPSET1: *mut u32 = (GPIO_BASE + 0x20) as *mut u32;
        const GPCLR1: *mut u32 = (GPIO_BASE + 0x2C) as *mut u32;
        const PIN_BIT: u32 = 1 << 10;  // GPIO 42 - 32

        // Init: set GPIO 42 as output
        let mut fsel = read_volatile(GPFSEL4);
        fsel &= !(7 << 6);
        fsel |= 1 << 6;
        write_volatile(GPFSEL4, fsel);

        // XXX Sense is inverted
        if on {
            write_volatile(GPCLR1, PIN_BIT);
        } else {
            write_volatile(GPSET1, PIN_BIT);
        }
    }

    /// Delay for `n` "time counts" (arbitrary).
    #[inline(never)]
    #[no_mangle]
    pub extern "C" fn _delay_n(n: usize) {
        for _ in 0..n {
            for _ in 0..0x100000_u32 { core::hint::spin_loop(); }
        }
    }
}

pub use raw::*;

/// Return the digits of `n` in base `base` as [usize]s.
fn digits(mut n: usize, base: usize) -> impl Iterator<Item = usize> {
    let mut m = 1;
    let mut q = n;
    while q >= base {
        q /= base;
        m *= base;
    }
    
    core::iter::from_fn(move || {
        if m == 0 {
            return None;
        }

        let d = n / m;
        n -= d * m;
        m /= base;
        Some(d)
    })
}

/// Blink the given code number as a binary number using
/// International Morse symbols and spacing. Dah for a one
/// bit, dit for a zero bit. Does a word delay before
/// proceeding if requested.
#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _blink_code(n: usize, word_delay: bool) {
    _set_led(false);
    if word_delay {
        _delay_n(7);
    }
    for b in digits(n, 2) {
        // XXX No way to fail here, so hopefully this
        // code only ever returns 0 or 1.
        if b == 1 {
            _set_led(true);
            _delay_n(3);
            _set_led(false);
            _delay_n(1);
        } else {
            _set_led(true);
            _delay_n(1);
            _set_led(false);
            _delay_n(3);
        }
    }
}
