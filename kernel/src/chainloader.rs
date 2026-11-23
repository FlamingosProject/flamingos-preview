// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! EL2, MMU-off ELF loader for UART and embedded stage-2 kernels.
//!
//! The loader owns RAM from 32 MiB upward; stage-2 destinations must be
//! below 32 MiB. The source ELF stays at 64 MiB for runtime symbol lookup.

use kernel_image::{ELF_STAGING_SIZE, ELF_STAGING_START, KernelImage};

#[cfg(feature = "embedded-kernel")]
static EMBEDDED_KERNEL: &[u8] = include_bytes!(env!("FLAMINGOS_KERNEL_ELF"));

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
#[cfg(not(feature = "embedded-kernel"))]
const FR_RXFE: u32 = 1 << 4;
const FR_TXFF: u32 = 1 << 5;

fn read32(addr: usize) -> u32 {
    // Safety: Only the board-selected, aligned GPIO/UART registers are passed
    // here, with the MMU off and other cores parked.
    // Need: Device registers require volatile accesses before drivers exist.
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

fn write32(addr: usize, value: u32) {
    // Safety: The board-selected register is mapped physical MMIO and this
    // boot core has exclusive access before the kernel starts.
    // Need: Device registers require volatile writes before drivers exist.
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

#[cfg(feature = "bsp_rpi3")]
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

#[cfg(not(feature = "embedded-kernel"))]
fn read_byte() -> u8 {
    while read32(UART_BASE + UART_FR) & FR_RXFE != 0 {
        core::hint::spin_loop();
    }
    read32(UART_BASE + UART_DR) as u8
}

#[cfg(not(feature = "embedded-kernel"))]
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

#[cfg(not(feature = "embedded-kernel"))]
const MINILOAD_BANNER: &str = concat!(
    r#"
 __  __ _      _ _                 _
|  \/  (_)_ _ (_) |   ___  __ _ __| |
| |\/| | | ' \| | |__/ _ \/ _` / _` |
|_|  |_|_|_||_|_|____\___/\__,_\__,_|

"#,
    "Flamingos chainloader ",
    env!("CARGO_PKG_VERSION"),
    "\n",
    env!("CARGO_PKG_DESCRIPTION"),
    "\nrev ",
    env!("FLAMINGOS_REVISION"),
    "\n\n",
);

/// Report a rejected image and stop without entering partially loaded code.
fn reject(reason: &str) -> ! {
    write_str("\n[ML] ELF rejected: ");
    write_str(reason);
    write_str("\n");
    flush();
    loop {
        core::hint::spin_loop();
    }
}

/// Copy the embedded source ELF to the retained staging area.
#[cfg(feature = "embedded-kernel")]
fn receive_image() -> usize {
    let size = EMBEDDED_KERNEL.len();
    if size == 0 || size > ELF_STAGING_SIZE as usize {
        reject("embedded image exceeds staging capacity");
    }
    // Safety: The linker bounds the relocated loader (including its embedded
    // ELF) below 64 MiB. This destination is reserved RAM at 64..96 MiB, with
    // no overlap, MMU/caches off, and only the boot core running.
    // Need: Move the ELF to physical RAM that survives stage-2 initialization.
    unsafe {
        core::ptr::copy_nonoverlapping(
            EMBEDDED_KERNEL.as_ptr(),
            ELF_STAGING_START as *mut u8,
            size,
        );
    }
    size
}

/// Receive an ELF using the existing size-prefixed serial transfer protocol.
#[cfg(not(feature = "embedded-kernel"))]
fn receive_image() -> usize {
    write_str(MINILOAD_BANNER);
    write_str("[ML] Requesting ELF\n");
    clear_rx();
    for _ in 0..3 {
        write_byte(3);
    }
    let size = u32::from_le_bytes([read_byte(), read_byte(), read_byte(), read_byte()]) as usize;
    if size == 0 || size > ELF_STAGING_SIZE as usize {
        reject("upload exceeds staging capacity");
    }
    write_str("OK");
    for offset in 0..size {
        // Safety: The checked upload length fits reserved RAM at 64..96 MiB.
        // There are no aliases or other cores accessing this byte yet.
        // Need: Populate the physical staging buffer before creating a slice.
        unsafe {
            core::ptr::write_volatile((ELF_STAGING_START as *mut u8).add(offset), read_byte());
        }
    }
    size
}

/// Load a validated ELF and enter its physical bootstrap with retained metadata.
///
/// Stage 1 keeps address translation disabled. Stage 2 receives the DTB in x0,
/// the retained ELF physical base in x1, and its byte count in x2; stage 2 owns
/// construction of its initial translation tables.
pub fn run() -> ! {
    init_uart();
    let size = receive_image();
    // Safety: receive_image initialized exactly these bytes in reserved RAM.
    // Stage-2 destinations are validated below 32 MiB, so loading cannot mutate
    // this immutable slice. The loader and DTB occupy a separate region.
    // Need: Borrow the physically received ELF for bounds-checked parsing.
    let bytes = unsafe { core::slice::from_raw_parts(ELF_STAGING_START as *const u8, size) };
    let image = KernelImage::parse(bytes).unwrap_or_else(|_| reject("invalid load layout"));
    // Validate metadata before any segment writes as well as the load layout.
    if image.symbols().is_err() {
        reject("invalid symbol metadata");
    }
    for segment in image.segments() {
        let destination = segment.physical_address as *mut u8;
        let file_size = segment.file_bytes.len();
        // Safety: parse checked each destination is RAM below 32 MiB (including
        // the zero-file boot stack at 0x10000..0x80000), disjoint from other segments,
        // source ELF, relocated loader, its stack and DTB. Only this core runs;
        // the loaded kernel is not executed or borrowed until all writes end.
        // Need: Materialize ELF segments and zero their BSS/heap/stack tails.
        unsafe {
            if file_size != 0 {
                core::ptr::copy_nonoverlapping(segment.file_bytes.as_ptr(), destination, file_size);
            }
            core::ptr::write_bytes(
                destination.add(file_size),
                0,
                segment.memory_size as usize - file_size,
            );
        }
    }
    write_str("\n[ML] ELF loaded; entering stage 2\n\n");
    flush();
    // Safety: Boot executes at EL2 with MMU/data caching disabled. All segment
    // writes are finished and the entry was validated in executable file data.
    // Invalidate old instruction contents before branching to the new program.
    // Need: Instruction synchronization is required after loading executable RAM.
    unsafe {
        core::arch::asm!("dsb sy", "ic iallu", "dsb sy", "isb", options(nostack));
    }
    // Safety: The linker reserves the copied DTB inside the relocated loader.
    // Need: Assembly and this handoff share the linker-defined physical buffer.
    unsafe extern "C" {
        static __device_tree_start: u8;
    }
    // Safety: parse verified a four-byte-aligned physical executable entry.
    // The trusted kernel bootstrap implements this three-register ABI and does
    // not return. The DTB and ELF remain reserved for its lifetime.
    // Need: Transfer control from the loader into the newly materialized kernel.
    let kernel: unsafe extern "C" fn(*const u8, usize, usize) -> ! =
        unsafe { core::mem::transmute(image.entry() as usize) };
    // Safety: All three arguments describe the preserved boot buffers above;
    // other cores are parked and the physical memory/cache contract still holds.
    // Need: Stage 2 takes ownership of the machine and cannot be called safely
    // as an ordinary Rust function.
    unsafe {
        kernel(
            core::ptr::addr_of!(__device_tree_start),
            ELF_STAGING_START as usize,
            size,
        )
    }
}
