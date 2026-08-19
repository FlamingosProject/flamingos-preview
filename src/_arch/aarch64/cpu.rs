// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Architectural processor code.
//!
//! # Orientation
//!
//! Since arch modules are imported into generic modules using the path attribute, the path of this
//! file is:
//!
//! crate::cpu::arch_cpu

use aarch64_cpu::asm;

#[cfg(feature = "test_build")]
use qemu_exit::QEMUExit;

#[cfg(feature = "test_build")]
const QEMU_EXIT_HANDLE: qemu_exit::AArch64 = qemu_exit::AArch64::new();

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Pause execution on the core.
#[inline(always)]
#[cfg_attr(feature = "test_build", allow(dead_code))]
pub fn wait_forever() -> ! {
    loop {
        asm::wfe()
    }
}

/// Make the host QEMU process exit with a failure status.
#[cfg(feature = "test_build")]
pub fn qemu_exit_failure() -> ! {
    QEMU_EXIT_HANDLE.exit_failure()
}

/// Make the host QEMU process exit successfully.
#[cfg(feature = "test_build")]
pub fn qemu_exit_success() -> ! {
    QEMU_EXIT_HANDLE.exit_success()
}
