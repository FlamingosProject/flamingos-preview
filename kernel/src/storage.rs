// SPDX-License-Identifier: MIT OR Apache-2.0

//! Storage-device adapters shared by filesystems and kernel code.
//!
//! The hardware driver exposes fixed-size SD sectors, while filesystems use
//! their own block-device traits. This module is the narrow translation layer:
//! it owns no cache and contains no partition or filesystem policy.

use crate::bsp::driver::{EMMC, EmmcError};
use exfat_embedded::BlockDevice;

const SECTOR_SIZE: usize = 512;

/// Errors translating filesystem block requests to the SD-card controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SdCardBlockDeviceError {
    /// The EMMC/SD controller rejected the operation.
    Controller(EmmcError),
    /// The filesystem supplied a buffer other than one 512-byte sector.
    InvalidBufferLength,
    /// The requested sector is outside the card or the controller's address range.
    AddressOutOfRange,
}

/// A filesystem-facing view of an initialized Raspberry Pi SD-card controller.
///
/// Construction snapshots the card capacity reported by its CSD. The adapter
/// borrows the kernel's static controller instance and serializes operations
/// through the controller's own lock.
pub struct SdCardBlockDevice {
    controller: &'static EMMC,
    sector_count: u64,
}

impl SdCardBlockDevice {
    /// Wrap an initialized controller and retain its reported capacity.
    ///
    /// The controller must have successfully completed [`EMMC::initialize`].
    pub fn new(controller: &'static EMMC) -> Result<Self, SdCardBlockDeviceError> {
        let sector_count = controller
            .sector_count()
            .map_err(SdCardBlockDeviceError::Controller)?;
        Ok(Self {
            controller,
            sector_count,
        })
    }

    fn block_index(&self, lba: u64) -> Result<u32, SdCardBlockDeviceError> {
        // Keep the filesystem's wider LBA type at this boundary. The current
        // controller command API deliberately uses a u32 block argument.
        if lba >= self.sector_count {
            return Err(SdCardBlockDeviceError::AddressOutOfRange);
        }
        u32::try_from(lba).map_err(|_| SdCardBlockDeviceError::AddressOutOfRange)
    }
}

impl BlockDevice for SdCardBlockDevice {
    type Error = SdCardBlockDeviceError;

    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }

    fn sector_count(&self) -> u64 {
        self.sector_count
    }

    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), Self::Error> {
        let block_index = self.block_index(lba)?;
        let block: &mut [u8; SECTOR_SIZE] = out
            .try_into()
            .map_err(|_| SdCardBlockDeviceError::InvalidBufferLength)?;
        self.controller
            .read_block(block_index, block)
            .map_err(SdCardBlockDeviceError::Controller)
    }

    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), Self::Error> {
        let block_index = self.block_index(lba)?;
        let block: &[u8; SECTOR_SIZE] = data
            .try_into()
            .map_err(|_| SdCardBlockDeviceError::InvalidBufferLength)?;
        self.controller
            .write_block(block_index, block)
            .map_err(SdCardBlockDeviceError::Controller)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // The blocking controller waits for DATA_DONE (including the card's
        // programming time) before returning, so no transfer remains queued.
        Ok(())
    }
}
