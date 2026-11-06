// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Top-level BSP file for the Raspberry Pi 3 and 4.

pub mod cpu;
pub mod driver;
pub mod exception;
pub mod memory;

use core::sync::atomic::{AtomicU8, Ordering};

// Bit 7 carries polarity; 0xff means the board has no supported direct ACT GPIO.
static DIRECT_ACT_LED: AtomicU8 = AtomicU8::new(0xff);

/// Record recognized direct-GPIO ACT wiring from a firmware compatible string.
pub(crate) fn record_act_led_board(compatible: &str) {
    let wiring = match compatible {
        "raspberrypi,model-zero-2-w" if cfg!(feature = "bsp_rpi3") => 29 | 0x80,
        "raspberrypi,4-model-b" if cfg!(feature = "bsp_rpi4") => 42,
        _ => return,
    };
    DIRECT_ACT_LED.store(wiring, Ordering::Relaxed);
}

/// Direct ACT LED pin and polarity for recognized boards; virtual GPIO is unsupported.
pub fn direct_act_led() -> Option<(u8, bool)> {
    match DIRECT_ACT_LED.load(Ordering::Relaxed) {
        0xff => None,
        wiring => Some((wiring & 0x7f, wiring & 0x80 != 0)),
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Board identification.
pub fn board_name() -> &'static str {
    #[cfg(feature = "bsp_rpi3")]
    {
        "Raspberry Pi 3"
    }

    #[cfg(feature = "bsp_rpi4")]
    {
        "Raspberry Pi 4"
    }
}
