// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! System console.

mod buffer_console;

use crate::synchronization::{self, NullLock};

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

        /// Write a string slice byte-for-byte.
        #[allow(unused)]
        fn write_str(&self, s: &str) {
            self.write_array(s.as_bytes());
        }

        /// Write a byte slice byte-for-byte.
        #[allow(unused)]
        fn write_array(&self, a: &[u8]) {
            for &b in a {
                self.write_byte(b);
            }
        }

        /// Block until the last buffered byte has been physically put on the TX wire.
        #[allow(unused)]
        fn flush(&self);
    }

    /// Raw console read functions. These never translate bytes.
    pub trait RawRead {
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

    /// Trait alias for a raw byte console.
    pub trait RawConsole: RawWrite + RawRead + Statistics {}

    /// Terminal write functions. These apply the selected line discipline.
    pub trait Write {
        /// Write a single terminal byte.
        #[allow(unused)]
        fn write_byte(&self, c: u8);

        /// Write a string slice.
        #[allow(unused)]
        fn write_str(&self, s: &str);

        /// Write a byte slice.
        #[allow(unused)]
        fn write_array(&self, a: &[u8]);

        /// Write Rust formatted output.
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;

        /// Flush the underlying raw console.
        #[allow(unused)]
        fn flush(&self);
    }

    /// Terminal read functions. These apply the selected line discipline.
    pub trait Read {
        /// Read a single cooked byte.
        #[allow(unused)]
        fn read_byte(&self) -> u8;

        /// Clear RX buffers, if any.
        #[allow(unused)]
        fn clear_rx(&self);
    }

    /// Trait alias for a terminal console.
    pub trait Console: Write + Read + Statistics {}
}

/// Output line discipline policy.
#[derive(Copy, Clone)]
pub struct OutputPolicy {
    /// If true, terminal output maps bare LF bytes to CRLF.
    pub map_lf_to_crlf: bool,
}

/// Input line discipline policy.
#[derive(Copy, Clone)]
pub struct InputPolicy {
    /// If true, terminal input maps CR bytes to LF.
    pub map_cr_to_lf: bool,
    /// Selects whether and how terminal input is echoed.
    pub echo: EchoPolicy,
}

/// Terminal echo behavior.
#[allow(unused)]
#[derive(Copy, Clone)]
pub enum EchoPolicy {
    /// Do not echo received input.
    Off,
    /// Echo received input bytes before input mapping.
    Raw,
    /// Echo received input bytes after input mapping through terminal output.
    Cooked,
}

/// The terminal console line discipline.
pub struct Terminal;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static CUR_RAW_CONSOLE: NullLock<&'static (dyn interface::RawConsole + Sync)> =
    NullLock::new(&buffer_console::BUFFER_CONSOLE);
static OUTPUT_POLICY: NullLock<OutputPolicy> = NullLock::new(OutputPolicy {
    map_lf_to_crlf: true,
});
static INPUT_POLICY: NullLock<InputPolicy> = NullLock::new(InputPolicy {
    map_cr_to_lf: false,
    echo: EchoPolicy::Off,
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

/// Register a new raw console.
pub fn register_console(new_console: &'static (dyn interface::RawConsole + Sync)) {
    CUR_RAW_CONSOLE.lock(|con| *con = new_console);
    buffer_console::BUFFER_CONSOLE.drain_to(new_console);
}

/// Return a reference to the currently registered raw console.
pub fn raw_console() -> &'static dyn interface::RawConsole {
    CUR_RAW_CONSOLE.lock(|con| *con)
}

/// Return a reference to the terminal console used by printing macros.
pub fn console() -> &'static dyn interface::Console {
    &TERMINAL
}

/// Set the terminal output policy.
#[allow(unused)]
pub fn set_output_policy(policy: OutputPolicy) {
    OUTPUT_POLICY.lock(|current| *current = policy);
}

/// Set the terminal input policy.
#[allow(unused)]
pub fn set_input_policy(policy: InputPolicy) {
    INPUT_POLICY.lock(|current| *current = policy);
}

impl interface::Write for Terminal {
    fn write_byte(&self, c: u8) {
        let mut prev_was_cr = false;
        self.write_text_byte(c, &mut prev_was_cr);
    }

    fn write_str(&self, s: &str) {
        let mut prev_was_cr = false;
        for b in s.bytes() {
            self.write_text_byte(b, &mut prev_was_cr);
        }
    }

    fn write_array(&self, a: &[u8]) {
        let mut prev_was_cr = false;
        for &b in a {
            self.write_text_byte(b, &mut prev_was_cr);
        }
    }

    fn write_fmt(&self, args: core::fmt::Arguments) -> core::fmt::Result {
        core::fmt::Write::write_fmt(
            &mut TerminalWriter {
                terminal: self,
                prev_was_cr: false,
            },
            args,
        )
    }

    fn flush(&self) {
        raw_console().flush();
    }
}

impl interface::Read for Terminal {
    fn read_byte(&self) -> u8 {
        let raw = raw_console().read_byte();
        let input_policy = INPUT_POLICY.lock(|policy| *policy);
        let cooked = if input_policy.map_cr_to_lf && raw == b'\r' {
            b'\n'
        } else {
            raw
        };

        match input_policy.echo {
            EchoPolicy::Off => {}
            EchoPolicy::Raw => raw_console().write_byte(raw),
            EchoPolicy::Cooked => interface::Write::write_byte(self, cooked),
        }

        cooked
    }

    fn clear_rx(&self) {
        raw_console().clear_rx();
    }
}

impl interface::Statistics for Terminal {
    fn bytes_written(&self) -> usize {
        raw_console().bytes_written()
    }

    fn bytes_read(&self) -> usize {
        raw_console().bytes_read()
    }
}

impl interface::Console for Terminal {}

impl core::fmt::Write for &dyn interface::Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        interface::Write::write_str(*self, s);
        Ok(())
    }
}
