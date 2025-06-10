// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! System console.

mod null_console;

use crate::synchronization::{self, NullLock};

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Console interfaces.
pub mod interface {
    use core::fmt;

    /// Console write functions.
    pub trait Write {
        /// Write a single byte.
        #[allow(unused)]
        fn write_byte(&self, c: u8);

        /// Write a string slice.
        #[allow(unused)]
        fn write_str(&self, s: &str);

        /// Write Rust formatted output.
        #[allow(unused)]
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;

        /// Block until the last buffered byte has been physically put on the TX wire.
        #[allow(unused)]
        fn flush(&self);
    }

    /// Console read functions.
    pub trait Read {
        /// Read a single byte.
        #[allow(unused)]
        fn read_byte(&self) -> u8;

        /// Clear RX buffers, if any.
        #[allow(unused)]
        fn clear_rx(&self);
    }

    /// Console statistics.
    pub trait Statistics {
        /// Return the number of bytes written.
        #[allow(unused)]
        fn bytes_written(&self) -> usize;

        /// Return the number of bytes read.
        #[allow(unused)]
        fn bytes_read(&self) -> usize;
    }

    // XXX Consoles should not be required to provide statistics.
    /// Trait alias for a full-fledged console.
    pub trait Console: Write + Read + Statistics {}
}

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static CUR_CONSOLE: NullLock<&'static (dyn interface::Console + Sync)> =
    NullLock::new(&null_console::NULL_CONSOLE);

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
use synchronization::interface::Mutex;

/// Register a new console.
pub fn register_console(new_console: &'static (dyn interface::Console + Sync)) {
    CUR_CONSOLE.lock(|con| *con = new_console);
}

/// Return a reference to the currently registered console.
///
/// This is the global console used by all printing macros.
pub fn console() -> &'static dyn interface::Console {
    CUR_CONSOLE.lock(|con| *con)
}
