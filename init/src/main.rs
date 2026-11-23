#![no_std]
#![no_main]

/// Blink the activity LED forever through the kernel's scalar syscall ABI.
///
/// # Safety
///
/// The caller must enter at EL1h with asynchronous exceptions masked,
/// a mapped 16-byte-aligned process stack, and EL1 access to the physical
/// counter enabled. The kernel must have installed its SVC handler.
///
/// # Need
///
/// This entry point receives execution context from the kernel, which
/// Rust cannot validate.
// Safety: The kernel loads this unique entry symbol using its C ABI.
// Need: The loader must be able to locate the unmangled entry point.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    loop {
        if flamingos_syscall::set_led(true) != flamingos_syscall::SUCCESS {
            panic!("LED on syscall failed");
        }
        delay();
        if flamingos_syscall::set_led(false) != flamingos_syscall::SUCCESS {
            panic!("LED off syscall failed");
        }
        delay();
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
