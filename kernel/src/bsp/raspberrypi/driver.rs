// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

// XXX This code uses unsafe initialization to set up a
// bunch of mutable globals. We have checked this and
// believe it's OK.
#![allow(static_mut_refs)]

//! BSP driver support.

use super::{exception, memory::map::mmio};
use crate::{
    bsp::device_driver,
    console, driver as generic_driver,
    exception::{self as generic_exception},
    memory,
    memory::mmu::MMIODescriptor,
};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub use crate::bsp::device_driver::Error as EmmcError;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

static mut PL011_UART: MaybeUninit<device_driver::PL011Uart> = MaybeUninit::uninit();
static mut GPIO: MaybeUninit<device_driver::GPIO> = MaybeUninit::uninit();
static mut MAILBOX: MaybeUninit<device_driver::Mailbox> = MaybeUninit::uninit();
static mut EMMC: MaybeUninit<device_driver::EMMC> = MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi3")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::InterruptController> =
    MaybeUninit::uninit();

#[cfg(feature = "bsp_rpi4")]
static mut INTERRUPT_CONTROLLER: MaybeUninit<device_driver::GICv2> = MaybeUninit::uninit();

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_uart() -> Result<(), &'static str> {
    unsafe {
        let mmio_descriptor = MMIODescriptor::new(mmio::PL011_UART_START, mmio::PL011_UART_SIZE);
        let virt_addr =
            memory::mmu::kernel_map_mmio(device_driver::PL011Uart::COMPATIBLE, &mmio_descriptor)?;

        PL011_UART.write(device_driver::PL011Uart::new(virt_addr));

        Ok(())
    }
}

/// This must be called only after successful init of the UART driver.
unsafe fn post_init_uart() -> Result<(), &'static str> {
    unsafe {
        console::register_console(PL011_UART.assume_init_ref());

        Ok(())
    }
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_gpio() -> Result<(), &'static str> {
    unsafe {
        let mmio_descriptor = MMIODescriptor::new(mmio::GPIO_START, mmio::GPIO_SIZE);
        let virt_addr =
            memory::mmu::kernel_map_mmio(device_driver::GPIO::COMPATIBLE, &mmio_descriptor)?;

        GPIO.write(device_driver::GPIO::new(virt_addr));

        Ok(())
    }
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_emmc() -> Result<(), &'static str> {
    unsafe {
        let mmio_descriptor = MMIODescriptor::new(mmio::EMMC_START, mmio::EMMC_SIZE);
        let virt_addr =
            memory::mmu::kernel_map_mmio(device_driver::EMMC::COMPATIBLE, &mmio_descriptor)?;

        EMMC.write(device_driver::EMMC::new(virt_addr));
        Ok(())
    }
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_mailbox() -> Result<(), &'static str> {
    unsafe {
        let mmio_descriptor = MMIODescriptor::new(mmio::MAILBOX_START, mmio::MAILBOX_SIZE);
        let virt_addr =
            memory::mmu::kernel_map_mmio(device_driver::Mailbox::COMPATIBLE, &mmio_descriptor)?;

        MAILBOX.write(device_driver::Mailbox::new(virt_addr));
        Ok(())
    }
}

/// This must be called only after successful init of the GPIO driver.
unsafe fn post_init_gpio() -> Result<(), &'static str> {
    unsafe {
        GPIO.assume_init_ref().map_pl011_uart();
        GPIO.assume_init_ref().map_emmc();
        Ok(())
    }
}

/// This must be called only after successful init of the memory subsystem.
#[cfg(feature = "bsp_rpi3")]
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    unsafe {
        let local_mmio_descriptor = MMIODescriptor::new(mmio::LOCAL_IC_START, mmio::LOCAL_IC_SIZE);
        let local_virt_addr = memory::mmu::kernel_map_mmio(
            device_driver::InterruptController::COMPATIBLE,
            &local_mmio_descriptor,
        )?;

        let periph_mmio_descriptor =
            MMIODescriptor::new(mmio::PERIPHERAL_IC_START, mmio::PERIPHERAL_IC_SIZE);
        let periph_virt_addr = memory::mmu::kernel_map_mmio(
            device_driver::InterruptController::COMPATIBLE,
            &periph_mmio_descriptor,
        )?;

        INTERRUPT_CONTROLLER.write(device_driver::InterruptController::new(
            local_virt_addr,
            periph_virt_addr,
        ));

        Ok(())
    }
}

/// This must be called only after successful init of the memory subsystem.
#[cfg(feature = "bsp_rpi4")]
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    unsafe {
        let gicd_mmio_descriptor = MMIODescriptor::new(mmio::GICD_START, mmio::GICD_SIZE);
        let gicd_virt_addr = memory::mmu::kernel_map_mmio("GICv2 GICD", &gicd_mmio_descriptor)?;

        let gicc_mmio_descriptor = MMIODescriptor::new(mmio::GICC_START, mmio::GICC_SIZE);
        let gicc_virt_addr = memory::mmu::kernel_map_mmio("GICV2 GICC", &gicc_mmio_descriptor)?;

        INTERRUPT_CONTROLLER.write(device_driver::GICv2::new(gicd_virt_addr, gicc_virt_addr));

        Ok(())
    }
}

/// This must be called only after successful init of the interrupt controller driver.
unsafe fn post_init_interrupt_controller() -> Result<(), &'static str> {
    unsafe {
        generic_exception::asynchronous::register_irq_manager(
            INTERRUPT_CONTROLLER.assume_init_ref(),
        );

        Ok(())
    }
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_uart() -> Result<(), &'static str> {
    unsafe {
        instantiate_uart()?;

        let uart_descriptor = generic_driver::DeviceDriverDescriptor::new(
            PL011_UART.assume_init_ref(),
            Some(post_init_uart),
            Some(exception::asynchronous::irq_map::PL011_UART),
        );
        generic_driver::driver_manager().register_driver(uart_descriptor);

        Ok(())
    }
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_gpio() -> Result<(), &'static str> {
    unsafe {
        instantiate_gpio()?;

        let gpio_descriptor = generic_driver::DeviceDriverDescriptor::new(
            GPIO.assume_init_ref(),
            Some(post_init_gpio),
            None,
        );
        generic_driver::driver_manager().register_driver(gpio_descriptor);

        Ok(())
    }
}

/// Register the SD-card host. Card discovery is explicit and does not make
/// kernel boot depend on removable media being present.
unsafe fn driver_emmc() -> Result<(), &'static str> {
    unsafe {
        instantiate_emmc()?;
        let descriptor =
            generic_driver::DeviceDriverDescriptor::new(EMMC.assume_init_ref(), None, None);
        generic_driver::driver_manager().register_driver(descriptor);
        Ok(())
    }
}

/// Register the VideoCore property mailbox used for platform discovery.
unsafe fn driver_mailbox() -> Result<(), &'static str> {
    unsafe {
        instantiate_mailbox()?;
        let descriptor =
            generic_driver::DeviceDriverDescriptor::new(MAILBOX.assume_init_ref(), None, None);
        generic_driver::driver_manager().register_driver(descriptor);
        Ok(())
    }
}

/// Function needs to ensure that driver registration happens only after correct instantiation.
unsafe fn driver_interrupt_controller() -> Result<(), &'static str> {
    unsafe {
        instantiate_interrupt_controller()?;

        let interrupt_controller_descriptor = generic_driver::DeviceDriverDescriptor::new(
            INTERRUPT_CONTROLLER.assume_init_ref(),
            Some(post_init_interrupt_controller),
            None,
        );
        generic_driver::driver_manager().register_driver(interrupt_controller_descriptor);

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Initialize the driver subsystem.
///
/// # Safety
///
/// See child function calls.
pub unsafe fn init() -> Result<(), &'static str> {
    unsafe {
        static INIT_DONE: AtomicBool = AtomicBool::new(false);
        if INIT_DONE.load(Ordering::Relaxed) {
            return Err("Init already done");
        }

        driver_uart()?;
        driver_gpio()?;
        driver_mailbox()?;
        driver_emmc()?;
        driver_interrupt_controller()?;

        INIT_DONE.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Return the board's SD-card host controller.
pub fn emmc() -> &'static device_driver::EMMC {
    // The BSP initializes this singleton before the driver manager runs.
    unsafe { EMMC.assume_init_ref() }
}

/// Return the ARM-to-VideoCore property mailbox.
pub fn mailbox() -> &'static device_driver::Mailbox {
    // The BSP initializes this singleton before the driver manager runs.
    unsafe { MAILBOX.assume_init_ref() }
}

/// Minimal code needed to bring up the console in QEMU (for testing only). This is often less steps
/// than on real hardware due to QEMU's abstractions.
#[cfg(feature = "test_build")]
pub fn qemu_bring_up_console() {
    use crate::cpu;

    unsafe {
        instantiate_uart().unwrap_or_else(|_| cpu::qemu_exit_failure());
        console::register_console(PL011_UART.assume_init_ref());
    };
}
