//! Scalar syscall ABI shared by the kernel and privileged init.
//!
//! AArch64 callers encode the syscall number in the 16-bit `svc` immediate
//! and pass its argument in x0. The signed result is returned in x0. All other
//! general registers and the caller's saved status are preserved. Init
//! currently runs at EL1h; this ABI does not provide privilege isolation.

#![no_std]

/// Set the board's activity LED to the logical state supplied in x0.
pub const SYS_SET_LED: u16 = 1;

/// The requested operation completed.
pub const SUCCESS: i64 = 0;
/// The argument is outside the syscall's accepted values.
pub const INVALID_ARGUMENT: i64 = -22;
/// No supported activity LED is available.
pub const NO_DEVICE: i64 = -19;
/// The syscall number is unknown.
pub const NOT_IMPLEMENTED: i64 = -38;

/// Invoke the scalar ABI and return the kernel's signed status.
///
/// The running kernel must implement this ABI before callers use it.
/// Unknown numbers and invalid scalar arguments return errors. The
/// kernel owns hardware addresses; this ABI accepts no pointers.
/// The const-generic number fits the instruction's 16-bit immediate field.
#[cfg(target_arch = "aarch64")]
#[inline]
pub fn call<const NUMBER: u16>(argument: u64) -> i64 {
    let result: u64;
    // Safety: the installed handler validates scalar inputs, preserves all
    // registers except x0, and restores the caller's status and stack.
    // Need: issuing a synchronous supervisor call requires assembly. Keep
    // the default memory effects because the handler may access devices.
    unsafe {
        core::arch::asm!(
            "svc #{number}",
            number = const NUMBER,
            inout("x0") argument => result,
        );
    }
    result as i64
}

/// Request the board activity LED's logical on/off state.
///
/// Returns [`SUCCESS`] or a negative status such as [`NO_DEVICE`].
#[cfg(target_arch = "aarch64")]
pub fn set_led(on: bool) -> i64 {
    call::<SYS_SET_LED>(u64::from(on))
}
