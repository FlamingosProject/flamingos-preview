// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

// Rust embedded logo for `make doc`.
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/rust-embedded/wg/master/assets/logo/ewg-logo-blue-white-on-transparent.png"
)]

//! The `kernel` binary.
#![no_main]
#![no_std]

extern crate alloc;

#[cfg(not(feature = "test_build"))]
use exfat_embedded::{FileSystem, Scratch};
#[cfg(not(feature = "test_build"))]
use libkernel::storage;
use libkernel::{bsp, cpu, driver, exception, info, memory, state, time};

/// Early init code.
///
/// When this code runs, virtual memory is already enabled.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - Printing will not work until the respective driver's MMIO is remapped.
#[unsafe(no_mangle)]
pub unsafe fn kernel_init() -> ! {
    unsafe {
        // The device tree is mapped now, so its parser can safely execute linked virtual code.
        #[cfg(not(feature = "chainloader"))]
        cpu::boot::arch_boot::process_device_tree();

        // Set up exception handlers.
        exception::handling_init();

        // Initialize memory subsystem, in particular memory
        // allocators.
        memory::init();

        // Initialize the timer subsystem.
        if let Err(x) = time::init() {
            panic!("Error initializing timer subsystem: {}", x);
        }

        // Initialize the BSP driver subsystem.
        if let Err(x) = bsp::driver::init() {
            panic!("Error initializing BSP driver subsystem: {}", x);
        }

        // Initialize all device drivers.
        driver::driver_manager().init_drivers_and_irqs();

        // Add records of how we mapped the kernel for later
        // logging once everything is up. Not currently used
        // otherwise.
        bsp::memory::mmu::kernel_add_mapping_records_for_precomputed();

        // Unmask interrupts on the boot CPU core.
        exception::asynchronous::local_irq_unmask();

        // Announce conclusion of the kernel_init() phase.
        state::state_manager().transition_to_single_core_main();

        // Transition from unsafe to safe.
        kernel_main()
    }
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    use alloc::boxed::Box;
    use core::time::Duration;

    info!("{}", libkernel::version());
    info!("Booting on: {}", bsp::board_name());

    unsafe {
        let cores_info = core::ptr::addr_of!(cpu::boot::arch_boot::CORES_INFO);
        let num_cores = (*cores_info).num_cores;
        info!("Cores:");
        for c in 0..num_cores {
            info!("    {}", (*cores_info).core_ids[c]);
        }
    }

    info!("MMU online:");
    memory::mmu::kernel_print_mappings();

    let (_, privilege_level) = exception::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    exception::asynchronous::print_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::time_manager().resolution().as_nanos()
    );

    info!("Drivers loaded:");
    driver::driver_manager().enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Kernel heap:");
    memory::heap_alloc::kernel_heap_allocator().print_usage();

    #[cfg(not(feature = "test_build"))]
    sdcard_smoke_test();

    time::time_manager().set_timeout_once(Duration::from_secs(5), Box::new(|| info!("Once 5")));
    time::time_manager().set_timeout_once(Duration::from_secs(2), Box::new(|| info!("Once 2")));
    time::time_manager()
        .set_timeout_periodic(Duration::from_secs(1), Box::new(|| info!("Periodic 1 sec")));

    info!("UART RX IRQs enabled");

    #[cfg(feature = "test_build")]
    cpu::qemu_exit_success();

    #[cfg(not(feature = "test_build"))]
    cpu::wait_forever();
}

/// Exercise the Chapter 22 storage stack without making media mandatory.
///
/// The raw sector CRC checks the controller path independently. The second
/// half mounts the first exFAT partition and verifies a known root-directory
/// file, covering partition discovery, directory lookup, and file reads.
#[cfg(not(feature = "test_build"))]
fn sdcard_smoke_test() {
    info!("SD card test: initializing");
    let sd = bsp::driver::emmc();

    match sd.initialize(bsp::driver::mailbox()) {
        Ok(()) => {}
        Err(bsp::driver::EmmcError::NoCard) => {
            libkernel::warn!("SD card test: no card present");
            return;
        }
        Err(error) => {
            libkernel::warn!("SD card test: initialization failed: {:?}", error);
            return;
        }
    }

    // Sector zero belongs to the whole disk (normally its MBR), not to the
    // exFAT volume within partition 2.
    let mut sector = [0_u8; 512];
    if let Err(error) = sd.read_block(0, &mut sector) {
        libkernel::warn!("SD card test: sector 0 read failed: {:?}", error);
        return;
    }

    info!(
        "SD card test: {} sectors, sector 0 CRC-32 = {:#010x}, four-bit bus = {}",
        sd.sector_count().unwrap_or(0),
        crc32(&sector),
        sd.four_bit_bus().unwrap_or(false)
    );

    // Hand the whole-disk block device to exfat-embedded. Its mount operation
    // discovers the partition before interpreting the exFAT boot region.
    let device = match storage::SdCardBlockDevice::new(sd) {
        Ok(device) => device,
        Err(error) => {
            libkernel::warn!("exFAT test: block-device setup failed: {:?}", error);
            return;
        }
    };
    // One caller-owned sector is enough workspace for mounting, directory
    // traversal, and this small file read; no allocation is required.
    let mut scratch_bytes = [0_u8; 512];
    let mut scratch = Scratch::new(&mut scratch_bytes);
    let mut filesystem = match FileSystem::mount(device, &mut scratch) {
        Ok(filesystem) => filesystem,
        Err(error) => {
            libkernel::warn!("exFAT test: mount failed: {:?}", error);
            return;
        }
    };
    let partition = filesystem.geometry().partition;
    info!(
        "exFAT test: mounted partition at LBA {} ({} sectors)",
        partition.first_lba, partition.sector_count
    );
    let mut file = match filesystem.open("123.txt", &mut scratch) {
        Ok(file) => file,
        Err(error) => {
            libkernel::warn!("exFAT test: opening 123.txt failed: {:?}", error);
            return;
        }
    };
    if file.len() != 4 {
        libkernel::warn!("exFAT test: 123.txt has unexpected size {}", file.len());
        return;
    }

    let mut contents = [0_u8; 4];
    match filesystem.read(&mut file, &mut contents, &mut scratch) {
        Ok(4) if contents == *b"123\n" => info!("exFAT test: 123.txt contains expected data"),
        Ok(bytes_read) => libkernel::warn!(
            "exFAT test: unexpected 123.txt contents (CRC-32 {:#010x}, {} bytes)",
            crc32(&contents[..bytes_read]),
            bytes_read
        ),
        Err(error) => libkernel::warn!("exFAT test: reading 123.txt failed: {:?}", error),
    }
}

/// IEEE CRC-32, reflected representation (`0xedb8_8320`).
#[cfg(not(feature = "test_build"))]
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
