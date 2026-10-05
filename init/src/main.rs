#![no_std]
#![no_main]

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    #[allow(clippy::empty_loop)]
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    #[allow(clippy::empty_loop)]
    loop {}
}
