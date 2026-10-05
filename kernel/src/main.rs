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
use libkernel::{bsp, cpu, driver, exception, info, memory, state, time};
#[cfg(not(feature = "test_build"))]
use libkernel::{program, storage};

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
    // Keep the owned image alive for the eventual process-creation step.
    let _program_image = load_boot_program();

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

/// Prepare root-level `init.elf` without making media or a valid image mandatory.
///
/// The returned image owns its segment data, allowing the local mount to be
/// dropped. Test builds skip this path because QEMU lacks the firmware mailbox.
#[cfg(not(feature = "test_build"))]
fn load_boot_program() -> Option<program::ProgramImage> {
    let sd = bsp::driver::emmc();
    match sd.initialize(bsp::driver::mailbox()) {
        Ok(()) => {}
        Err(bsp::driver::EmmcError::NoCard) => {
            libkernel::warn!("ELF loader: no card present");
            return None;
        }
        Err(error) => {
            libkernel::warn!("ELF loader: card initialization failed: {:?}", error);
            return None;
        }
    }
    let device = match storage::SdCardBlockDevice::new(sd) {
        Ok(device) => device,
        Err(error) => {
            libkernel::warn!("ELF loader: block-device setup failed: {:?}", error);
            return None;
        }
    };
    let mut scratch_bytes = [0_u8; 512];
    let mut scratch = Scratch::new(&mut scratch_bytes);
    let mut filesystem = match FileSystem::mount(device, &mut scratch) {
        Ok(filesystem) => filesystem,
        Err(error) => {
            libkernel::warn!("ELF loader: mount failed: {:?}", error);
            return None;
        }
    };
    match program::load(&mut filesystem, "init.elf", &mut scratch) {
        Ok(image) => {
            info!(
                "ELF loader: init.elf entry {:#x}, {} segments",
                image.entry,
                image.segments.len()
            );
            for segment in &image.segments {
                info!(
                    "ELF segment: address {:#x}, {} bytes, permissions {:?}",
                    segment.virtual_address,
                    segment.data.len(),
                    segment.permissions
                );
            }
            Some(image)
        }
        Err(error) => {
            libkernel::warn!("ELF loader: init.elf failed: {:?}", error);
            None
        }
    }
}
