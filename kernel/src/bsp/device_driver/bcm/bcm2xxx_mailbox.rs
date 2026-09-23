// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Adapted from joeferner/rpi-hal src/mailbox.rs at commit
// 258c4b9d778f4598eeb05aa97682fe9994fd5a16.

//! Blocking VideoCore property-mailbox driver.
//!
//! The firmware processes one tagged request in shared RAM. Since kernel RAM
//! is cacheable, the request is cleaned before it is submitted and invalidated
//! after the firmware returns it.

use crate::{
    bsp::device_driver::common::MMIODerefWrapper,
    driver,
    exception::asynchronous::IRQNumber,
    memory::{self, Address, Virtual},
    synchronization::{IRQSafeNullLock, interface::Mutex},
};
use aarch64_cpu::asm::barrier;
use core::{
    arch::asm,
    mem::{size_of, size_of_val},
};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::{ReadOnly, WriteOnly},
};

register_structs! {
    #[allow(non_snake_case)]
    RegisterBlock {
        (0x00 => READ: ReadOnly<u32>),
        (0x04 => _reserved1),
        (0x18 => STATUS: ReadOnly<u32>),
        (0x1c => _reserved2),
        (0x20 => WRITE: WriteOnly<u32>),
        (0x24 => _reserved3),
        (0x38 => STATUS1: ReadOnly<u32>),
        (0x3c => @END),
    }
}

type Registers = MMIODerefWrapper<RegisterBlock>;

const CHANNEL_PROPERTY_TAGS: u32 = 8;
const TAG_GET_CLOCK_RATE: u32 = 0x0003_0002;
const TAG_SET_CLOCK_STATE: u32 = 0x0003_8001;
const TAG_SET_POWER_STATE: u32 = 0x0002_8001;
const RESPONSE_BIT: u32 = 1 << 31;
const STATUS_FULL: u32 = 1 << 31;
const STATUS_EMPTY: u32 = 1 << 30;
const VALUE_WORDS: usize = 2;
const MESSAGE_WORDS: usize = 2 + 3 + VALUE_WORDS + 1;

/// Firmware clock identifiers used by the SD-card controllers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ClockId {
    /// Classic EMMC controller used by Pi 3-family boards.
    Emmc = 1,
    /// EMMC2 controller connected to the Pi 4 SD-card slot.
    Emmc2 = 12,
}

/// Failures reported by the VideoCore property interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxError {
    /// The request buffer could not be translated to a physical address.
    AddressTranslation,
    /// The firmware returned a response for a different request buffer.
    AddressMismatch,
    /// The firmware rejected the complete property message.
    RequestFailed,
    /// The firmware did not answer the requested property tag.
    TagNotAnswered,
    /// The response was shorter than the property requires.
    ShortResponse,
    /// The requested clock does not exist or has no usable rate.
    ClockUnavailable,
    /// The firmware reported that a requested state was not established.
    StateUnavailable,
}

struct MailboxInner {
    registers: Registers,
}

/// Raspberry Pi ARM-to-VideoCore mailbox.
pub struct Mailbox {
    inner: IRQSafeNullLock<MailboxInner>,
}

impl MailboxInner {
    const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        unsafe {
            Self {
                registers: Registers::new(mmio_start_addr),
            }
        }
    }

    fn property_call(
        &mut self,
        tag: u32,
        request: &[u32],
        response_words: usize,
    ) -> Result<[u32; VALUE_WORDS], MailboxError> {
        debug_assert!(request.len() <= VALUE_WORDS);
        debug_assert!(response_words <= VALUE_WORDS);

        // A complete cache line of private storage prevents invalidation from
        // discarding unrelated dirty stack data that happens to share a line.
        #[repr(C, align(64))]
        struct Buffer {
            words: [u32; 16],
        }

        let mut buffer = Buffer { words: [0; 16] };
        buffer.words[0] = (MESSAGE_WORDS * size_of::<u32>()) as u32;
        buffer.words[1] = 0;
        buffer.words[2] = tag;
        buffer.words[3] = (VALUE_WORDS * size_of::<u32>()) as u32;
        buffer.words[4] = size_of_val(request) as u32;
        buffer.words[5..5 + request.len()].copy_from_slice(request);
        // The end tag at words[5 + VALUE_WORDS] was zero-initialized.

        let virtual_address = &buffer as *const Buffer as usize;
        let physical_address =
            memory::mmu::try_kernel_virt_addr_to_phys_addr(Address::new(virtual_address))
                .map_err(|_| MailboxError::AddressTranslation)?
                .as_usize();
        let physical_address =
            u32::try_from(physical_address).map_err(|_| MailboxError::AddressTranslation)?;

        clean_cache_line(virtual_address);
        let echoed = self.call_raw(physical_address);
        invalidate_cache_line(virtual_address);

        if echoed != physical_address {
            return Err(MailboxError::AddressMismatch);
        }
        if buffer.words[1] & RESPONSE_BIT == 0 {
            return Err(MailboxError::RequestFailed);
        }
        if buffer.words[4] & RESPONSE_BIT == 0 {
            return Err(MailboxError::TagNotAnswered);
        }
        if (buffer.words[4] & !RESPONSE_BIT) < (response_words * size_of::<u32>()) as u32 {
            return Err(MailboxError::ShortResponse);
        }

        let mut response = [0; VALUE_WORDS];
        response[..response_words].copy_from_slice(&buffer.words[5..5 + response_words]);
        Ok(response)
    }

    fn call_raw(&mut self, physical_address: u32) -> u32 {
        // Channel 8 is the documented exception to the other mailbox
        // channels' VC-bus-address rule: property tags carry the plain ARM
        // physical address.
        let message = (physical_address & !0xf) | CHANNEL_PROPERTY_TAGS;

        while self.registers.STATUS1.get() & STATUS_FULL != 0 {}
        self.registers.WRITE.set(message);

        loop {
            while self.registers.STATUS.get() & STATUS_EMPTY != 0 {}
            let response = self.registers.READ.get();
            if response & 0xf == CHANNEL_PROPERTY_TAGS {
                return response & !0xf;
            }
        }
    }
}

impl Mailbox {
    /// Compatibility string used by the kernel driver manager.
    pub const COMPATIBLE: &'static str = "BCM VideoCore mailbox";

    /// Construct a mailbox over an already mapped MMIO register window.
    ///
    /// # Safety
    ///
    /// `mmio_start_addr` must name the exclusively owned mailbox registers.
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        unsafe {
            Self {
                inner: IRQSafeNullLock::new(MailboxInner::new(mmio_start_addr)),
            }
        }
    }

    /// Return the firmware-configured base rate of `clock`, in Hz.
    pub fn clock_rate_hz(&self, clock: ClockId) -> Result<u32, MailboxError> {
        self.inner.lock(|inner| {
            let response = inner.property_call(TAG_GET_CLOCK_RATE, &[clock as u32], 2)?;
            if response[0] != clock as u32 || response[1] == 0 {
                return Err(MailboxError::ClockUnavailable);
            }
            Ok(response[1])
        })
    }

    /// Ask the firmware to enable or disable a peripheral clock.
    pub fn set_clock_state(&self, clock: ClockId, enabled: bool) -> Result<(), MailboxError> {
        self.inner.lock(|inner| {
            let state = u32::from(enabled);
            let response = inner.property_call(TAG_SET_CLOCK_STATE, &[clock as u32, state], 2)?;
            if response[0] != clock as u32 || response[1] & 0b11 != state {
                return Err(MailboxError::StateUnavailable);
            }
            Ok(())
        })
    }

    /// Power the SD-card domain on and wait for it to become stable.
    pub fn power_on_sd_card(&self) -> Result<(), MailboxError> {
        self.inner.lock(|inner| {
            // Device 0 is the SD-card power domain; state bits request ON and WAIT.
            let response = inner.property_call(TAG_SET_POWER_STATE, &[0, 0b11], 2)?;
            if response[0] != 0 || response[1] & 0b11 != 1 {
                return Err(MailboxError::StateUnavailable);
            }
            Ok(())
        })
    }
}

impl driver::interface::DeviceDriver for Mailbox {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}

fn clean_cache_line(virtual_address: usize) {
    unsafe { asm!("dc cvac, {address}", address = in(reg) virtual_address) };
    barrier::dsb(barrier::SY);
}

fn invalidate_cache_line(virtual_address: usize) {
    barrier::dsb(barrier::SY);
    unsafe { asm!("dc ivac, {address}", address = in(reg) virtual_address) };
    barrier::dsb(barrier::SY);
}
