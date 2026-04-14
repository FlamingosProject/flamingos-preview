// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! DWARF-based backtrace support using the `unwinding` crate.

use crate::println;
use core::ffi::c_void;
use unwinding::custom_eh_frame_finder::{
    set_custom_eh_frame_finder, EhFrameFinder, FrameInfo, FrameInfoKind,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

/// Provides the `.eh_frame_hdr` location to the unwinder via linker-exported symbols.
struct KernelEhFrameFinder;

unsafe impl EhFrameFinder for KernelEhFrameFinder {
    fn find(&self, _pc: usize) -> Option<FrameInfo> {
        unsafe extern "C" {
            static __eh_frame_hdr_start: u8;
        }

        Some(FrameInfo {
            text_base: None,
            kind: FrameInfoKind::EhFrameHdr(unsafe { &__eh_frame_hdr_start } as *const u8 as usize),
        })
    }
}

static FINDER: KernelEhFrameFinder = KernelEhFrameFinder;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Register the kernel's EH frame data with the unwinder.
///
/// Must be called early in boot, before anything that might panic.
pub fn init() {
    set_custom_eh_frame_finder(&FINDER).expect("EH frame finder already set");
}

/// Walk the call stack and print each frame's instruction pointer.
pub fn print_backtrace() {
    use unwinding::abi::{
        UnwindContext, UnwindReasonCode, UnwindTraceFn, _Unwind_Backtrace, _Unwind_GetIP,
    };

    extern "C" fn trace_fn(ctx: &UnwindContext<'_>, arg: *mut c_void) -> UnwindReasonCode {
        let frame_num = unsafe { &mut *(arg as *mut u32) };
        let ip = _Unwind_GetIP(ctx);
        // Use \r for raw UART output.
        println!("  #{}: {:#018x}\r", frame_num, ip);
        *frame_num += 1;
        UnwindReasonCode::NO_REASON
    }

    println!("\r\nBacktrace:\r");
    let mut frame_num: u32 = 0;
    _Unwind_Backtrace(
        trace_fn as UnwindTraceFn,
        &mut frame_num as *mut u32 as *mut c_void,
    );
}
