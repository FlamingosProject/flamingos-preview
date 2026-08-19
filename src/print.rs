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
    #[allow(unused_imports)]
    use console::interface::Write;

    console::console().write_fmt(args).unwrap();
}

/// Prints without a newline.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::print::_print(format_args!($($arg)*));
    }};
}

/// Prints with a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {{
        $crate::print::_print(format_args!($($arg)*));
        $crate::println!();
    }};
}

/// Prints a warning with a newline.
#[macro_export]
macro_rules! warn {
    ($string:expr) => {{
        $crate::println!(concat!("[W] ", $string));
    }};
    ($format_string:expr, $($arg:tt)*) => {{
        $crate::println!(concat!("[W] ", $format_string), $($arg)*);
    }};
}
