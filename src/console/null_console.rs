// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

//! Null console.

use super::interface;
use core::fmt;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub struct NullConsole;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

pub static NULL_CONSOLE: NullConsole = NullConsole {};

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl interface::Write for NullConsole {
    fn write_byte(&self, _c: u8) {}

    fn write_str(&self, _s: &str) {}

    fn write_fmt(&self, _args: fmt::Arguments) -> fmt::Result {
        fmt::Result::Ok(())
    }

    fn flush(&self) {}
}

impl interface::Read for NullConsole {
    fn clear_rx(&self) {}

    // XXX The interface should be fixed to allow some way
    // to indicate that no data is available to read.
    fn read_byte(&self) -> u8 {
        b' '
    }
}

impl interface::Statistics for NullConsole {
    fn bytes_written(&self) -> usize {
        0
    }

    fn bytes_read(&self) -> usize {
        0
    }
}

impl interface::Console for NullConsole {}
