// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! System console.

use crate::{bsp, synchronization, synchronization::NullLock};

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Console interfaces.
pub mod interface {
    use core::fmt;

    /// Raw console write functions. These never translate bytes.
    pub trait RawWrite {
        /// Write a single byte.
        fn write_byte(&self, c: u8);

        /// Flush any pending output.
        #[allow(unused)]
        fn flush(&self) {}
    }

    /// Console statistics.
    pub trait Statistics {
        /// Return the number of characters written.
        fn chars_written(&self) -> usize {
            0
        }
    }

    /// Trait alias for a raw byte console.
    pub trait RawConsole: RawWrite + Statistics {}

    /// Terminal write functions. These apply the selected line discipline.
    pub trait Write {
        /// Write Rust formatted output.
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;
    }

    /// Trait alias for a full-fledged terminal console.
    pub trait All: Write + Statistics {}
}

/// Output line discipline policy.
#[derive(Copy, Clone)]
pub struct OutputPolicy {
    /// If true, terminal output maps bare LF bytes to CRLF.
    pub map_lf_to_crlf: bool,
}

/// The terminal console line discipline.
pub struct Terminal;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static OUTPUT_POLICY: NullLock<OutputPolicy> = NullLock::new(OutputPolicy {
    map_lf_to_crlf: true,
});
static TERMINAL: Terminal = Terminal;

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

use synchronization::interface::Mutex;

impl Terminal {
    fn write_text_byte(&self, c: u8, prev_was_cr: &mut bool) {
        let output_policy = OUTPUT_POLICY.lock(|policy| *policy);

        if output_policy.map_lf_to_crlf && c == b'\n' && !*prev_was_cr {
            raw_console().write_byte(b'\r');
        }

        raw_console().write_byte(c);
        *prev_was_cr = c == b'\r';
    }
}

struct TerminalWriter<'a> {
    terminal: &'a Terminal,
    prev_was_cr: bool,
}

impl core::fmt::Write for TerminalWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.terminal.write_text_byte(b, &mut self.prev_was_cr);
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Return a reference to the raw console device.
pub fn raw_console() -> &'static dyn interface::RawConsole {
    bsp::console::raw_console()
}

/// Return a reference to the terminal console used by printing macros.
pub fn console() -> &'static dyn interface::All {
    &TERMINAL
}

/// Set the terminal output policy.
#[allow(unused)]
pub fn set_output_policy(policy: OutputPolicy) {
    OUTPUT_POLICY.lock(|current| *current = policy);
}

impl interface::Write for Terminal {
    fn write_fmt(&self, args: core::fmt::Arguments) -> core::fmt::Result {
        core::fmt::Write::write_fmt(
            &mut TerminalWriter {
                terminal: self,
                prev_was_cr: false,
            },
            args,
        )
    }
}

impl interface::Statistics for Terminal {
    fn chars_written(&self) -> usize {
        raw_console().chars_written()
    }
}

impl interface::All for Terminal {}
