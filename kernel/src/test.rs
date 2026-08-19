// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2026 Bart Massey

//! Support for self-terminating QEMU test kernels.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::cpu;

static EXPECTED_PANIC: AtomicBool = AtomicBool::new(false);

/// Mark the next kernel panic as the successful outcome of the current test.
///
/// Any panic before this function is called remains a test failure.
pub fn expect_panic() {
    EXPECTED_PANIC.store(true, Ordering::Release);
}

/// Exit QEMU according to whether the current panic was expected.
pub(crate) fn exit_panic() -> ! {
    if EXPECTED_PANIC.swap(false, Ordering::AcqRel) {
        cpu::qemu_exit_success()
    } else {
        cpu::qemu_exit_failure()
    }
}
