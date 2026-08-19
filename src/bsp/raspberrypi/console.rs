// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP console facilities.

use crate::{console, synchronization, synchronization::NullLock};

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

/// Return a reference to the raw console device.
pub fn raw_console() -> &'static dyn console::interface::RawConsole {
    &QEMU_OUTPUT
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::Mutex;

impl console::interface::RawWrite for QEMUOutput {
    fn write_byte(&self, c: u8) {
        self.inner.lock(|inner| inner.write_byte(c));
    }
}

impl console::interface::Statistics for QEMUOutput {
    fn chars_written(&self) -> usize {
        self.inner.lock(|inner| inner.chars_written)
    }
}

impl console::interface::RawConsole for QEMUOutput {}
