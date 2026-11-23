//! Dispatch the scalar syscall ABI used by privileged init.

use crate::bsp;
use flamingos_syscall::{INVALID_ARGUMENT, NO_DEVICE, NOT_IMPLEMENTED, SUCCESS, SYS_SET_LED};

/// Validate a syscall before touching its kernel-owned device.
pub(crate) fn dispatch(number: u16, argument: u64) -> i64 {
    match number {
        SYS_SET_LED => {
            let on = match argument {
                0 => false,
                1 => true,
                _ => return INVALID_ARGUMENT,
            };
            match bsp::driver::set_activity_led(on) {
                Ok(()) => SUCCESS,
                Err(_) => NO_DEVICE,
            }
        }
        _ => NOT_IMPLEMENTED,
    }
}
