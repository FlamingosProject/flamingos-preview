// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! DWARF-based backtrace support using the `unwinding` crate.

use crate::println;
use core::ffi::c_void;

pub fn print_backtrace() {
    use unwinding::abi::{
        _Unwind_Backtrace, _Unwind_GetIP, UnwindContext, UnwindReasonCode, UnwindTraceFn,
    };

    extern "C" fn trace_fn(ctx: &UnwindContext<'_>, arg: *mut c_void) -> UnwindReasonCode {
        let frame_num = unsafe { &mut *(arg as *mut u32) };
        let ip = _Unwind_GetIP(ctx);
        println!("  #{}: {:#018x}", frame_num, ip);
        *frame_num += 1;
        UnwindReasonCode::NO_REASON
    }

    println!();
    println!("Backtrace:");
    let mut frame_num: u32 = 0;
    _Unwind_Backtrace(
        trace_fn as UnwindTraceFn,
        &mut frame_num as *mut u32 as *mut c_void,
    );
}
