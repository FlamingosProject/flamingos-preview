#![no_std]
#![no_main]

#[path = "../../kernel/src/_arch/aarch64/cpu/led.rs"]
mod led;

/// Configures the supplied GPIO pin and blinks it forever.
///
/// # Safety
///
/// The caller must enter at EL1h with asynchronous exceptions masked,
/// a mapped 16-byte-aligned process stack, and EL1 access to the physical
/// counter enabled. `gpio_base` must be a mapped GPIO register block and
/// `pin` must identify a valid pin in that block. In `qemu_test` builds,
/// `completion` must be a nonzero address of a trusted kernel function with
/// the signature `unsafe extern "C" fn(usize) -> !`.
///
/// # Need
///
/// This entry point receives raw hardware and execution context from the
/// kernel, which Rust cannot validate.
// Safety: The kernel loads this unique entry symbol using its C ABI.
// Need: The loader must be able to locate the unmangled entry point.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(
    gpio_base: usize,
    pin: usize,
    active_low: usize,
    completion: usize,
) -> ! {
    #[cfg_attr(feature = "qemu_test", allow(clippy::never_loop))]
    loop {
        // Safety: The kernel supplies a valid mapped GPIO block and an
        // exclusively owned board LED pin under the entry contract.
        // Need: Operate the LED using the same raw helper as boot debugging.
        unsafe { led::set_led(gpio_base, pin as u8, active_low != 0, true) };
        delay();
        // Safety: The entry contract still holds; no other code accesses GPIO.
        // Need: Turn the LED off with the shared boot-debugging implementation.
        unsafe { led::set_led(gpio_base, pin as u8, active_low != 0, false) };
        delay();
        #[cfg(feature = "qemu_test")]
        {
            // Safety: The caller guarantees that completion is a trusted,
            // nonzero function address with this ABI and signature.
            // Need: The test harness provides its callback as an address.
            let callback: unsafe extern "C" fn(usize) -> ! =
                unsafe { core::mem::transmute(completion) };
            // Safety: The callback and its argument are supplied by the kernel
            // under the entry point contract.
            // Need: Control must return to the kernel test harness.
            unsafe { callback(gpio_base) }
        }
        #[cfg(not(feature = "qemu_test"))]
        let _ = completion;
    }
}

/// Waits approximately a quarter of a second using the physical counter.
fn delay() {
    let frequency: u64;
    let start: u64;
    // Safety: The entry contract guarantees EL1 access to these registers.
    // Need: The generic timer provides a board-independent time source.
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) frequency, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mrs {0}, cntpct_el0", out(reg) start, options(nomem, nostack, preserves_flags));
    }
    loop {
        let now: u64;
        // Safety: The entry contract guarantees EL1 access to the counter.
        // Need: Reading the counter measures elapsed hardware time.
        unsafe {
            core::arch::asm!("mrs {0}, cntpct_el0", out(reg) now, options(nomem, nostack, preserves_flags))
        };
        if now.wrapping_sub(start) >= frequency / 4 {
            break;
        }
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
