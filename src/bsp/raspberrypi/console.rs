// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP console facilities.

use crate::console;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

/// A mystical, magical device for generating QEMU output out of the void.
struct QEMUOutput;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Return a terminal console backed by raw QEMU output.
pub fn console() -> impl console::interface::Write {
    console::Terminal::new(
        QEMUOutput,
        console::OutputPolicy {
            map_lf_to_crlf: true,
        },
    )
}

impl console::interface::RawWrite for QEMUOutput {
    fn write_byte(&self, b: u8) {
        unsafe {
            core::ptr::write_volatile(0x3F20_1000 as *mut u8, b);
        }
    }
}
