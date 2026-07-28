// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021-2023 Andre Richter <andre.o.richter@gmail.com>

//! Architectural boot code.
//!
//! # Orientation
//!
//! Since arch modules are imported into generic modules using the path attribute, the path of this
//! file is:
//!
//! crate::cpu::boot::arch_boot

mod led_debug;

use core::arch::global_asm;

#[cfg(feature = "boot_trace")]
const BOOT_TRACE: u64 = 1;
#[cfg(not(feature = "boot_trace"))]
const BOOT_TRACE: u64 = 0;

// Assembly counterpart to this file.
global_asm!(
    include_str!("boot.s"),
    CONST_CORE_ID_MASK = const 0b11,
    CONST_BOOT_TRACE = const BOOT_TRACE,
);

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Blink a fatal early-boot error code forever.
#[inline(never)]
#[no_mangle]
pub unsafe extern "C" fn _panic_code(code: usize) -> ! {
    loop {
        led_debug::_blink_code(code, true);
    }
}

/// The Rust entry of the `kernel` binary.
///
/// The function is called from the assembly `_start` function.
#[no_mangle]
pub unsafe extern "C" fn _start_rust(_device_tree: *const u8) -> ! {
    #[cfg(feature = "boot_trace")]
    led_debug::_blink_code(3, true);

    crate::kernel_init()
}
