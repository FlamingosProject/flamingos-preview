// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Adapted from joeferner/rpi-hal src/sd.rs at commit
// 258c4b9d778f4598eeb05aa97682fe9994fd5a16.

//! Blocking PIO driver for the Raspberry Pi SD-card host controller.
//!
//! Initialization uses the VideoCore property mailbox for platform-specific
//! power and clock setup, then performs the SD card-identification sequence.
//! The data path deliberately stops at 512-byte logical sectors: partition and
//! filesystem policy belong in adapters above this driver.
//!
//! Transfers currently use synchronous, single-block programmed I/O. The
//! controller lock therefore protects both the initialized-card state and the
//! complete command/data transaction.

use crate::{
    bsp::device_driver::{ClockId, Mailbox, common::MMIODerefWrapper},
    driver,
    exception::asynchronous::IRQNumber,
    memory::{Address, Virtual},
    synchronization::{IRQSafeNullLock, interface::Mutex},
    time,
};
use core::time::Duration;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::ReadWrite,
};

register_structs! {
    #[allow(non_snake_case)]
    RegisterBlock {
        (0x00 => ARG2: ReadWrite<u32>),
        (0x04 => BLKSIZECNT: ReadWrite<u32>),
        (0x08 => ARG1: ReadWrite<u32>),
        (0x0c => CMDTM: ReadWrite<u32>),
        (0x10 => RESP0: ReadWrite<u32>),
        (0x14 => RESP1: ReadWrite<u32>),
        (0x18 => RESP2: ReadWrite<u32>),
        (0x1c => RESP3: ReadWrite<u32>),
        (0x20 => DATA: ReadWrite<u32>),
        (0x24 => STATUS: ReadWrite<u32>),
        (0x28 => CONTROL0: ReadWrite<u32>),
        (0x2c => CONTROL1: ReadWrite<u32>),
        (0x30 => INTERRUPT: ReadWrite<u32>),
        (0x34 => IRPT_MASK: ReadWrite<u32>),
        (0x38 => IRPT_EN: ReadWrite<u32>),
        (0x3c => CONTROL2: ReadWrite<u32>),
        (0x40 => @END),
    }
}

type Registers = MMIODerefWrapper<RegisterBlock>;

const BLOCK_SIZE: usize = 512;
const SETUP_CLOCK_HZ: u32 = 400_000;
const TRANSFER_CLOCK_HZ: u32 = 25_000_000;

// These packed values are written directly to SDHCI's Command and Transfer
// Mode register. CMD_NEED_APP is our own marker; command() removes it after
// automatically issuing CMD55 for an application-specific command.
const CMD_GO_IDLE: u32 = 0x0000_0000;
const CMD_ALL_SEND_CID: u32 = 0x0201_0000;
const CMD_SEND_REL_ADDR: u32 = 0x0302_0000;
const CMD_CARD_SELECT: u32 = 0x0703_0000;
const CMD_SEND_IF_COND: u32 = 0x0802_0000;
const CMD_SEND_CSD: u32 = 0x0901_0000;
const CMD_SET_BLOCKLEN: u32 = 0x1002_0000;
const CMD_READ_SINGLE: u32 = 0x1122_0010;
const CMD_WRITE_SINGLE: u32 = 0x1822_0000;
const CMD_APP_CMD: u32 = 0x370a_0000;
const CMD_NEED_APP: u32 = 0x8000_0000;
const CMD_SEND_OP_COND: u32 = 0x2902_0000 | CMD_NEED_APP;
const CMD_SEND_SCR: u32 = 0x3322_0010 | CMD_NEED_APP;
const CMD_SET_BUS_WIDTH: u32 = 0x0602_0000 | CMD_NEED_APP;

const STATUS_CMD_INHIBIT: u32 = 1 << 0;
const STATUS_DAT_INHIBIT: u32 = 1 << 1;
const CONTROL0_4BIT: u32 = 1 << 1;
// SDHCI Power Control byte: select 3.3 V and enable bus power. BCM2711's
// EMMC2 needs this explicitly even though the classic controller does not.
#[cfg(feature = "bsp_rpi4")]
const CONTROL0_POWER_3V3: u32 = 0x0f00;
const CONTROL1_CLK_INTLEN: u32 = 1 << 0;
const CONTROL1_CLK_STABLE: u32 = 1 << 1;
const CONTROL1_CLK_EN: u32 = 1 << 2;
const CONTROL1_DATA_TIMEOUT_MAX: u32 = 0b1110 << 16;
const CONTROL1_SRST_HC: u32 = 1 << 24;
const CONTROL1_SRST_CMD: u32 = 1 << 25;

const INT_CMD_DONE: u32 = 1 << 0;
const INT_DATA_DONE: u32 = 1 << 1;
const INT_WRITE_RDY: u32 = 1 << 4;
const INT_READ_RDY: u32 = 1 << 5;
const INT_ERR_SUMMARY: u32 = 1 << 15;
const INT_CMD_TIMEOUT: u32 = 1 << 16;
const INT_ERROR_MASK: u32 = 0x017e_8000;

const ACMD41_ARG_HC: u32 = 0x51ff_8000;
const ACMD41_CMD_COMPLETE: u32 = 1 << 31;
const ACMD41_VOLTAGE: u32 = 0x00ff_8000;
const ACMD41_CMD_CCS: u32 = 1 << 30;
const SCR_SD_BUS_WIDTH_4: u32 = 0x0000_0400;

/// One logical sector transferred by an SD memory card.
pub type Block = [u8; BLOCK_SIZE];

/// Errors reported while initializing or accessing an SD card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The platform firmware could not power or clock the controller.
    Firmware(super::bcm2xxx_mailbox::MailboxError),
    /// A controller or card state transition did not finish in time.
    Timeout,
    /// A command or data transfer set an SDHCI error status bit.
    Card {
        /// Snapshot of the SDHCI interrupt status register.
        interrupt: u32,
        /// Packed command value active when the error occurred.
        command: u32,
    },
    /// The card rejected the voltage range offered by the host.
    UnsupportedVoltage,
    /// The SD-card slot did not answer card-identification commands.
    NoCard,
    /// No card has been initialized yet.
    NotInitialized,
    /// A byte-addressed SDSC request overflowed its 32-bit argument.
    AddressOverflow,
    /// The card reported a CSD layout this driver does not support.
    UnsupportedCsdVersion(u8),
    /// The card's CSD described an invalid or unrepresentable capacity.
    InvalidCsd,
}

#[derive(Clone, Copy)]
struct CardState {
    /// Relative card address in the upper 16 bits, ready for command arguments.
    relative_address: u32,
    /// Whether command arguments use sectors (true) or byte offsets (false).
    high_capacity: bool,
    /// Whether both card and host were successfully switched to four-bit mode.
    four_bit_bus: bool,
    /// Capacity decoded from the CSD, in 512-byte logical sectors.
    sector_count: u64,
}

/// Mutable controller state serialized by the public [`EMMC`] wrapper.
struct EMMCInner {
    registers: Registers,
    card: Option<CardState>,
}

/// Raspberry Pi Arasan EMMC/EMMC2 controller.
pub struct EMMC {
    inner: IRQSafeNullLock<EMMCInner>,
}

impl EMMCInner {
    const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        unsafe {
            Self {
                registers: Registers::new(mmio_start_addr),
                card: None,
            }
        }
    }

    fn initialize(&mut self, mailbox: &Mailbox) -> Result<(), Error> {
        self.card = None;

        // Firmware owns the SoC-level power and clock gates. Asking it for the
        // live base rate avoids making a board- or firmware-specific guess
        // when calculating the SD clock divisor.
        mailbox.power_on_sd_card().map_err(Error::Firmware)?;
        #[cfg(feature = "bsp_rpi3")]
        let clock_id = ClockId::Emmc;
        #[cfg(feature = "bsp_rpi4")]
        let clock_id = ClockId::Emmc2;
        #[cfg(feature = "bsp_rpi4")]
        mailbox
            .set_clock_state(clock_id, true)
            .map_err(Error::Firmware)?;
        let base_clock_hz = mailbox.clock_rate_hz(clock_id).map_err(Error::Firmware)?;

        // Reset the complete host circuit before programming its clocks.
        self.registers.CONTROL1.set(CONTROL1_SRST_HC);
        wait_for(100_000, || {
            self.registers.CONTROL1.get() & CONTROL1_SRST_HC == 0
        })?;

        #[cfg(feature = "bsp_rpi4")]
        self.registers
            .CONTROL0
            .set(self.registers.CONTROL0.get() | CONTROL0_POWER_3V3);

        self.registers
            .CONTROL1
            .set(self.registers.CONTROL1.get() | CONTROL1_CLK_INTLEN | CONTROL1_DATA_TIMEOUT_MAX);
        spin_ms(10);
        self.set_clock(base_clock_hz, SETUP_CLOCK_HZ)?;
        self.registers.IRPT_MASK.set(u32::MAX);
        self.registers.IRPT_EN.set(0);

        // Begin the protocol-defined identification sequence at <= 400 kHz.
        let mut card = CardState {
            relative_address: 0,
            high_capacity: false,
            four_bit_bus: false,
            sector_count: 0,
        };
        self.command(&card, CMD_GO_IDLE, 0)?;
        if let Err(error) = self.command(&card, CMD_SEND_IF_COND, 0x1aa) {
            return Err(self.classify_missing_card(&card, error));
        }

        let start = time::time_manager().uptime();
        let response = loop {
            let response = self.command(&card, CMD_SEND_OP_COND, ACMD41_ARG_HC)?;
            if response & ACMD41_CMD_COMPLETE != 0 {
                break response;
            }
            if elapsed_us(start) > 1_000_000 {
                return Err(Error::Timeout);
            }
            spin_ms(10);
        };
        if response & ACMD41_VOLTAGE == 0 {
            return Err(Error::UnsupportedVoltage);
        }
        card.high_capacity = response & ACMD41_CMD_CCS != 0;

        // Obtain an address and capacity before selecting the card and raising
        // the bus to its conservative default transfer frequency.
        self.command(&card, CMD_ALL_SEND_CID, 0)?;
        card.relative_address = self.command(&card, CMD_SEND_REL_ADDR, 0)? & 0xffff_0000;
        let csd = self.command_response_136(CMD_SEND_CSD, card.relative_address)?;
        card.sector_count = decode_csd_sector_count(csd)?;
        self.set_clock(base_clock_hz, TRANSFER_CLOCK_HZ)?;
        self.command(&card, CMD_CARD_SELECT, card.relative_address)?;
        if !card.high_capacity {
            self.command(&card, CMD_SET_BLOCKLEN, BLOCK_SIZE as u32)?;
        }
        // Four-bit mode is an optimization. A failed SCR read or a card that
        // does not advertise it leaves the standards-mandated one-bit mode.
        card.four_bit_bus = self.negotiate_four_bit_bus(&card).unwrap_or(false);
        self.card = Some(card);
        Ok(())
    }

    fn classify_missing_card(&mut self, card: &CardState, original: Error) -> Error {
        // A silent CMD8 may mean either an empty slot or an old, pre-SD-2.0
        // card. Only a bare command timeout is worth probing with CMD55,
        // which all SD protocol versions understand.
        let Error::Card { interrupt, .. } = original else {
            return original;
        };
        if interrupt & INT_CMD_TIMEOUT == 0 || interrupt & INT_ERROR_MASK != INT_ERR_SUMMARY {
            return original;
        }

        // SDHCI requires the command circuit to be reset after a command
        // error. Without this, the confirming CMD55 never starts and merely
        // reaches the driver's software timeout.
        self.registers
            .CONTROL1
            .set(self.registers.CONTROL1.get() | CONTROL1_SRST_CMD);
        if wait_for(10_000, || {
            self.registers.CONTROL1.get() & CONTROL1_SRST_CMD == 0
        })
        .is_err()
        {
            return original;
        }

        match self.command(card, CMD_APP_CMD, 0) {
            Err(Error::Card { interrupt, .. }) if interrupt & INT_CMD_TIMEOUT != 0 => Error::NoCard,
            _ => original,
        }
    }

    fn read_block(&mut self, block_index: u32, block: &mut Block) -> Result<(), Error> {
        let card = self.card.ok_or(Error::NotInitialized)?;
        self.start_transfer(&card, CMD_READ_SINGLE, block_index)?;
        self.wait_interrupt(INT_READ_RDY, CMD_READ_SINGLE)?;
        // The FIFO is 32 bits wide; SD sector bytes are transferred in
        // little-endian word order by this controller.
        for chunk in block.as_chunks_mut::<4>().0 {
            chunk.copy_from_slice(&self.registers.DATA.get().to_le_bytes());
        }
        Ok(())
    }

    fn write_block(&mut self, block_index: u32, block: &Block) -> Result<(), Error> {
        let card = self.card.ok_or(Error::NotInitialized)?;
        self.start_transfer(&card, CMD_WRITE_SINGLE, block_index)?;
        self.wait_interrupt(INT_WRITE_RDY, CMD_WRITE_SINGLE)?;
        // Fill the 32-bit FIFO after the controller announces write space.
        for chunk in block.as_chunks::<4>().0 {
            self.registers.DATA.set(u32::from_le_bytes(*chunk));
        }
        // DATA_DONE is raised after the card finishes programming, not merely
        // after the last word enters the host FIFO.
        self.wait_interrupt(INT_DATA_DONE, CMD_WRITE_SINGLE)?;
        Ok(())
    }

    fn start_transfer(
        &mut self,
        card: &CardState,
        command: u32,
        block_index: u32,
    ) -> Result<(), Error> {
        wait_for(100_000, || {
            self.registers.STATUS.get() & STATUS_DAT_INHIBIT == 0
        })?;
        self.registers.BLKSIZECNT.set((1 << 16) | BLOCK_SIZE as u32);
        // SDHC/SDXC commands carry an LBA; byte-addressed cards carry the
        // corresponding byte offset.
        let argument = if card.high_capacity {
            block_index
        } else {
            block_index
                .checked_mul(BLOCK_SIZE as u32)
                .ok_or(Error::AddressOverflow)?
        };
        self.command(card, command, argument).map(|_| ())
    }

    fn negotiate_four_bit_bus(&mut self, card: &CardState) -> Result<bool, Error> {
        // ACMD51 returns the eight-byte SD Configuration Register as a small
        // data transfer. ACMD6 then switches the card before we switch the
        // matching controller bit.
        wait_for(100_000, || {
            self.registers.STATUS.get() & STATUS_DAT_INHIBIT == 0
        })?;
        self.registers.BLKSIZECNT.set((1 << 16) | 8);
        self.command(card, CMD_SEND_SCR, 0)?;
        self.wait_interrupt(INT_READ_RDY, CMD_SEND_SCR)?;
        let first_word = self.registers.DATA.get();
        let _second_word = self.registers.DATA.get();
        if first_word & SCR_SD_BUS_WIDTH_4 == 0 {
            return Ok(false);
        }
        self.command(card, CMD_SET_BUS_WIDTH, 2)?;
        self.registers
            .CONTROL0
            .set(self.registers.CONTROL0.get() | CONTROL0_4BIT);
        Ok(true)
    }

    fn command(&mut self, card: &CardState, code: u32, argument: u32) -> Result<u32, Error> {
        if code & CMD_NEED_APP != 0 {
            self.command(card, CMD_APP_CMD, card.relative_address)?;
        }
        let code = code & !CMD_NEED_APP;
        wait_for(100_000, || {
            self.registers.STATUS.get() & STATUS_CMD_INHIBIT == 0
        })?;
        self.registers.INTERRUPT.set(self.registers.INTERRUPT.get());
        self.registers.ARG1.set(argument);
        self.registers.CMDTM.set(code);
        self.wait_interrupt(INT_CMD_DONE, code)?;
        Ok(self.registers.RESP0.get())
    }

    fn command_response_136(&mut self, code: u32, argument: u32) -> Result<[u32; 4], Error> {
        wait_for(100_000, || {
            self.registers.STATUS.get() & STATUS_CMD_INHIBIT == 0
        })?;
        self.registers.INTERRUPT.set(self.registers.INTERRUPT.get());
        self.registers.ARG1.set(argument);
        self.registers.CMDTM.set(code);
        self.wait_interrupt(INT_CMD_DONE, code)?;

        // SDHCI drops the response's CRC/end byte and stores each successive
        // 32-bit piece right-shifted by eight. Reassemble the CSD as four
        // big-endian words covering CSD bits 127..0.
        let r0 = self.registers.RESP0.get();
        let r1 = self.registers.RESP1.get();
        let r2 = self.registers.RESP2.get();
        let r3 = self.registers.RESP3.get();
        Ok([
            (r3 << 8) | (r2 >> 24),
            (r2 << 8) | (r1 >> 24),
            (r1 << 8) | (r0 >> 24),
            r0 << 8,
        ])
    }

    fn wait_interrupt(&mut self, mask: u32, command: u32) -> Result<u32, Error> {
        let start = time::time_manager().uptime();
        loop {
            let interrupt = self.registers.INTERRUPT.get();
            let consumed = interrupt & (mask | INT_ERROR_MASK);
            if consumed != 0 {
                self.registers.INTERRUPT.set(consumed);
                if interrupt & INT_ERROR_MASK != 0 {
                    return Err(Error::Card { interrupt, command });
                }
                return Ok(interrupt);
            }
            if elapsed_us(start) > 1_000_000 {
                return Err(Error::Timeout);
            }
        }
    }

    fn set_clock(&mut self, base_hz: u32, target_hz: u32) -> Result<(), Error> {
        wait_for(100_000, || {
            self.registers.STATUS.get() & (STATUS_CMD_INHIBIT | STATUS_DAT_INHIBIT) == 0
        })?;
        self.registers
            .CONTROL1
            .set(self.registers.CONTROL1.get() & !CONTROL1_CLK_EN);
        spin_ms(10);
        let divisor = clock_divider(base_hz, target_hz);
        let mut control = self.registers.CONTROL1.get() & !((0xff << 8) | (0x3 << 6));
        control |= (divisor & 0xff) << 8;
        control |= ((divisor >> 8) & 0x3) << 6;
        self.registers.CONTROL1.set(control | CONTROL1_CLK_EN);
        spin_ms(10);
        wait_for(100_000, || {
            self.registers.CONTROL1.get() & CONTROL1_CLK_STABLE != 0
        })
    }
}

impl EMMC {
    /// Compatibility string used by the kernel driver manager.
    pub const COMPATIBLE: &'static str = "BCM Arasan EMMC SD host";

    /// Construct a controller over an already mapped MMIO register window.
    ///
    /// # Safety
    ///
    /// `mmio_start_addr` must name an exclusively owned EMMC register block.
    pub const unsafe fn new(mmio_start_addr: Address<Virtual>) -> Self {
        unsafe {
            Self {
                inner: IRQSafeNullLock::new(EMMCInner::new(mmio_start_addr)),
            }
        }
    }

    /// Power the controller, query its base clock, and initialize the inserted card.
    pub fn initialize(&self, mailbox: &Mailbox) -> Result<(), Error> {
        self.inner.lock(|inner| inner.initialize(mailbox))
    }

    /// Return whether initialization negotiated the four-bit data bus.
    pub fn four_bit_bus(&self) -> Result<bool, Error> {
        self.inner.lock(|inner| {
            inner
                .card
                .map(|card| card.four_bit_bus)
                .ok_or(Error::NotInitialized)
        })
    }

    /// Return the initialized card's capacity in 512-byte logical sectors.
    pub fn sector_count(&self) -> Result<u64, Error> {
        self.inner.lock(|inner| {
            inner
                .card
                .map(|card| card.sector_count)
                .ok_or(Error::NotInitialized)
        })
    }

    /// Read one 512-byte logical sector.
    pub fn read_block(&self, block_index: u32, block: &mut Block) -> Result<(), Error> {
        self.inner
            .lock(|inner| inner.read_block(block_index, block))
    }

    /// Write one 512-byte logical sector and wait until it is committed.
    pub fn write_block(&self, block_index: u32, block: &Block) -> Result<(), Error> {
        self.inner
            .lock(|inner| inner.write_block(block_index, block))
    }
}

impl driver::interface::DeviceDriver for EMMC {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}

fn spin_ms(milliseconds: u64) {
    time::time_manager().spin_for(Duration::from_millis(milliseconds));
}

fn elapsed_us(start: Duration) -> u128 {
    (time::time_manager().uptime() - start).as_micros()
}

fn wait_for(timeout_us: u128, mut condition: impl FnMut() -> bool) -> Result<(), Error> {
    let start = time::time_manager().uptime();
    while !condition() {
        if elapsed_us(start) > timeout_us {
            return Err(Error::Timeout);
        }
    }
    Ok(())
}

/// Decode CSD v1 (SDSC) or v2 (SDHC/SDXC) capacity into 512-byte sectors.
fn decode_csd_sector_count(csd: [u32; 4]) -> Result<u64, Error> {
    let csd_structure = (csd[0] >> 30) as u8;
    let sector_count = match csd_structure {
        0 => {
            // CSD v1:
            //   bytes = (C_SIZE + 1) * 2^(C_SIZE_MULT + 2) * 2^READ_BL_LEN.
            let read_block_len = (csd[1] >> 16) & 0xf;
            let device_size = ((csd[1] & 0x3ff) << 2) | (csd[2] >> 30);
            let size_multiplier = (csd[2] >> 15) & 0x7;
            let block_len = 1_u64.checked_shl(read_block_len).ok_or(Error::InvalidCsd)?;
            let block_count = u64::from(device_size + 1)
                .checked_shl(size_multiplier + 2)
                .ok_or(Error::InvalidCsd)?;
            let capacity_bytes = block_count
                .checked_mul(block_len)
                .ok_or(Error::InvalidCsd)?;
            if !capacity_bytes.is_multiple_of(BLOCK_SIZE as u64) {
                return Err(Error::InvalidCsd);
            }
            capacity_bytes / BLOCK_SIZE as u64
        }
        1 => {
            // CSD v2 expresses C_SIZE in fixed 512-KiB units, or 1024 of
            // the 512-byte logical sectors exposed by this driver.
            let device_size = ((csd[1] & 0x3f) << 16) | (csd[2] >> 16);
            u64::from(device_size + 1) * 1024
        }
        version => return Err(Error::UnsupportedCsdVersion(version)),
    };

    if sector_count == 0 {
        return Err(Error::InvalidCsd);
    }
    Ok(sector_count)
}

/// Return the 10-bit SDHCI divisor field for a clock no faster than `target_hz`.
///
/// This controller accepts power-of-two divisors. The register stores half of
/// the effective divisor, split across two bit fields by [`EMMCInner::set_clock`].
/// The calculation was ported from Circle through `rpi-hal`.
fn clock_divider(base_hz: u32, target_hz: u32) -> u32 {
    let required = base_hz.saturating_add(target_hz - 1) / target_hz;
    let total_divisor = required.next_power_of_two().max(2);
    (total_divisor / 2).min(0x3ff)
}
