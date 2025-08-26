// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Printing.

use crate::console;
use core::fmt;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    console::console().write_fmt(args).unwrap();
}

/// Prints without a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        ($crate::print::_print(format_args!($($arg)*)));
    }};
}

/// Prints with a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => {{
        $crate::print::_print(format_args!($($arg)*));
        $crate::println!();
    }};
}

/// Prints an info, with a newline.
#[macro_export]
macro_rules! info {
    ($string:expr) => ({
        let timestamp = $crate::time::time_manager().uptime();

        $crate::println!(
            concat!("[  {:>3}.{:06}] \r\n", $string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
        );
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::time_manager().uptime();

        $crate::println!(
            concat!("[  {:>3}.{:06}] \r\n", $format_string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            $($arg)*
        );
    })
}

/// Prints a warning, with a newline.
#[macro_export]
macro_rules! warn {
    ($string:expr) => ({
        let timestamp = $crate::time::time_manager().uptime();

        $crate::println!(
            concat!("[W {:>3}.{:06}] \r\n", $string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
        );
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::time_manager().uptime();

        $crate::println!(
            concat!("[W {:>3}.{:06}] \r\n", $format_string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            $($arg)*
        );
    })
}
