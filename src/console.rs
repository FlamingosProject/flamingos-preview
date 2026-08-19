// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! System console.

use crate::bsp;

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

    /// Terminal write functions. These apply the selected line discipline.
    pub trait Write {
        /// Write Rust formatted output.
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;
    }
}

/// Output line discipline policy.
#[derive(Copy, Clone)]
pub struct OutputPolicy {
    /// If true, terminal output maps bare LF bytes to CRLF.
    pub map_lf_to_crlf: bool,
}

/// Terminal output line discipline.
pub struct Terminal<R> {
    raw: R,
    output_policy: OutputPolicy,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl<R> Terminal<R>
where
    R: interface::RawWrite,
{
    pub const fn new(raw: R, output_policy: OutputPolicy) -> Self {
        Self { raw, output_policy }
    }

    fn write_text_byte(&self, c: u8, prev_was_cr: &mut bool) {
        if self.output_policy.map_lf_to_crlf && c == b'\n' && !*prev_was_cr {
            self.raw.write_byte(b'\r');
        }

        self.raw.write_byte(c);
        *prev_was_cr = c == b'\r';
    }
}

struct TerminalWriter<'a, R>
where
    R: interface::RawWrite,
{
    terminal: &'a Terminal<R>,
    prev_was_cr: bool,
}

impl<R> core::fmt::Write for TerminalWriter<'_, R>
where
    R: interface::RawWrite,
{
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

/// Return a terminal console.
pub fn console() -> impl interface::Write {
    bsp::console::console()
}

impl<R> interface::Write for Terminal<R>
where
    R: interface::RawWrite,
{
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
