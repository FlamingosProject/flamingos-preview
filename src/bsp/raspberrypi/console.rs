// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP console facilities.

use crate::{console, synchronization, synchronization::NullLock};
use core::fmt;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

/// A mystical, magical device for generating QEMU output out of the void.
///
/// The mutex protected part.
struct QEMUOutputInner {
    chars_written: usize,
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The main struct.
pub struct QEMUOutput {
    inner: NullLock<QEMUOutputInner>,
}

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static QEMU_OUTPUT: QEMUOutput = QEMUOutput::new();

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl QEMUOutputInner {
    const fn new() -> QEMUOutputInner {
        QEMUOutputInner { chars_written: 0 }
    }

    /// Send a character.
    fn write_byte(&mut self, c: u8) {
        unsafe {
            core::ptr::write_volatile(0x3F20_1000 as *mut u8, c);
        }

        self.chars_written += 1;
    }
}

/// Implementing `core::fmt::Write` enables usage of the `format_args!` macros, which in turn are
/// used to implement the `kernel`'s `print!` and `println!` macros. By implementing `write_str()`,
/// we get `write_fmt()` automatically.
///
/// The function takes an `&mut self`, so it must be implemented for the inner struct.
///
/// See [`src/print.rs`].
///
/// [`src/print.rs`]: ../../print/index.html
impl fmt::Write for QEMUOutputInner {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            // Convert newline to carrige return + newline.
            if c == b'\n' {
                self.write_byte(b'\r')
            }

            self.write_byte(c);
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl QEMUOutput {
    /// Create a new instance.
    pub const fn new() -> QEMUOutput {
        QEMUOutput {
            inner: NullLock::new(QEMUOutputInner::new()),
        }
    }
}

/// Return a reference to the console.
pub fn console() -> &'static dyn console::interface::All {
    &QEMU_OUTPUT
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

/// Passthrough of `args` to the `core::fmt::Write` implementation, but guarded by a Mutex to
/// serialize access.
impl console::interface::Write for QEMUOutput {
    fn write_fmt(&self, args: core::fmt::Arguments) -> fmt::Result {
        // Fully qualified syntax for the call to `core::fmt::Write::write_fmt()` to increase
        // readability.
        self.inner.lock(|inner| fmt::Write::write_fmt(inner, args))
    }
}

impl console::interface::Statistics for QEMUOutput {
    fn chars_written(&self) -> usize {
        self.inner.lock(|inner| inner.chars_written)
    }
}

impl console::interface::All for QEMUOutput {}
