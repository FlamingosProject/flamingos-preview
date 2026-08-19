// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Minimal EL2, MMU-off UART chainloader.

const PAYLOAD_LOAD_ADDR: usize = 0x80000;

#[cfg(feature = "bsp_rpi3")]
const GPIO_BASE: usize = 0x3f20_0000;
#[cfg(feature = "bsp_rpi3")]
const UART_BASE: usize = 0x3f20_1000;

#[cfg(feature = "bsp_rpi4")]
const GPIO_BASE: usize = 0xfe20_0000;
#[cfg(feature = "bsp_rpi4")]
const UART_BASE: usize = 0xfe20_1000;

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_IBRD: usize = 0x24;
const UART_FBRD: usize = 0x28;
const UART_LCR_H: usize = 0x2c;
const UART_CR: usize = 0x30;
const UART_ICR: usize = 0x44;

const FR_BUSY: u32 = 1 << 3;
const FR_RXFE: u32 = 1 << 4;
const FR_TXFF: u32 = 1 << 5;

fn read32(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

fn write32(addr: usize, value: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

fn short_delay() {
    for _ in 0..150 {
        core::hint::spin_loop();
    }
}

fn init_gpio() {
    let gpfsel1 = GPIO_BASE + 0x04;
    let mut value = read32(gpfsel1);
    value &= !((0b111 << 12) | (0b111 << 15));
    value |= (0b100 << 12) | (0b100 << 15);
    write32(gpfsel1, value);

    #[cfg(feature = "bsp_rpi3")]
    {
        write32(GPIO_BASE + 0x94, 0);
        short_delay();
        write32(GPIO_BASE + 0x98, (1 << 14) | (1 << 15));
        short_delay();
        write32(GPIO_BASE + 0x94, 0);
        write32(GPIO_BASE + 0x98, 0);
    }

    #[cfg(feature = "bsp_rpi4")]
    {
        let pulls = GPIO_BASE + 0xe4;
        let value = read32(pulls) & !((0b11 << 28) | (0b11 << 30));
        write32(pulls, value);
    }
}

fn init_uart() {
    init_gpio();
    flush();
    write32(UART_BASE + UART_CR, 0);
    write32(UART_BASE + UART_ICR, 0x7ff);
    write32(UART_BASE + UART_IBRD, 3);
    write32(UART_BASE + UART_FBRD, 16);
    write32(UART_BASE + UART_LCR_H, (0b11 << 5) | (1 << 4));
    write32(UART_BASE + UART_CR, (1 << 0) | (1 << 8) | (1 << 9));
}

fn write_byte(byte: u8) {
    while read32(UART_BASE + UART_FR) & FR_TXFF != 0 {
        core::hint::spin_loop();
    }
    write32(UART_BASE + UART_DR, u32::from(byte));
}

fn write_str(string: &str) {
    for byte in string.bytes() {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}

fn read_byte() -> u8 {
    while read32(UART_BASE + UART_FR) & FR_RXFE != 0 {
        core::hint::spin_loop();
    }
    read32(UART_BASE + UART_DR) as u8
}

fn clear_rx() {
    while read32(UART_BASE + UART_FR) & FR_RXFE == 0 {
        let _ = read32(UART_BASE + UART_DR);
    }
}

fn flush() {
    while read32(UART_BASE + UART_FR) & FR_BUSY != 0 {
        core::hint::spin_loop();
    }
}

/// Receive a kernel at the firmware load address and enter it with the preserved device tree.
pub fn run() -> ! {
    init_uart();
    write_str("\nMiniLoad\n\n[ML] Requesting binary\n");
    clear_rx();
    for _ in 0..3 {
        write_byte(3);
    }

    let mut size = u32::from(read_byte());
    size |= u32::from(read_byte()) << 8;
    size |= u32::from(read_byte()) << 16;
    size |= u32::from(read_byte()) << 24;
    write_str("OK");

    let load_addr = PAYLOAD_LOAD_ADDR as *mut u8;
    for offset in 0..size {
        unsafe { core::ptr::write_volatile(load_addr.add(offset as usize), read_byte()) };
    }

    write_str("\n[ML] Loaded! Executing the payload now\n\n");
    flush();

    extern "C" {
        static __device_tree_start: u8;
    }
    let kernel: extern "C" fn(*const u8) -> ! = unsafe { core::mem::transmute(PAYLOAD_LOAD_ADDR) };
    kernel(core::ptr::addr_of!(__device_tree_start))
}
