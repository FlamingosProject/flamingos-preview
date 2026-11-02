//! Sparse exFAT boot images and a read-only in-memory block device.
//!
//! The header contains a version, logical byte size, and record count. Sorted
//! records contain a sector number and 512 literal bytes. Omitted sectors read
//! as zeros. Runtime mounting borrows the blob without allocating or expanding
//! empty disk space; only the host image builder uses an allocation.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "build")]
extern crate alloc;
#[cfg(feature = "build")]
use alloc::vec::Vec;
use exfat_embedded::BlockDevice;

const MAGIC: &[u8; 8] = b"FSPARSE1";
const HEADER_SIZE: usize = 16;
const SECTOR_SIZE: usize = 512;
const RECORD_SIZE: usize = 4 + SECTOR_SIZE;

/// Maximum logical disk size; omitted sectors consume no kernel heap or payload.
pub const MAX_IMAGE_SIZE: usize = 64 * 1024 * 1024;

/// A sparse boot image could not be constructed or validated.
#[derive(Debug, Eq, PartialEq)]
pub enum ImageError {
    /// Header, length, alignment, record order, or sector bounds are invalid.
    InvalidImage,
    /// The logical disk exceeds the configured limit.
    TooLarge,
    /// The host allocator could not reserve image or record storage.
    AllocationFailed,
    /// Host-side exFAT formatting or file creation failed.
    Filesystem,
}

/// Invalid sector request or attempted write to a read-only image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiskError {
    /// A sector buffer must contain exactly 512 bytes.
    InvalidBufferLength,
    /// The requested sector is not in this image.
    OutOfBounds,
    /// Embedded filesystems cannot be modified.
    ReadOnly,
}

fn word(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Borrows a validated sparse disk blob and exposes read-only sector access.
///
/// No expanded disk buffer or lookup index is allocated. Each read uses binary
/// search over fixed-size records and fills absent sectors with zeros.
pub struct ReadOnlyDisk<'a> {
    records: &'a [u8],
    size: usize,
}

impl<'a> ReadOnlyDisk<'a> {
    /// Validate a sparse boot image without allocating.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError`] for malformed headers, excessive logical size,
    /// incomplete/trailing records, duplicate/unsorted records, or invalid LBAs.
    pub fn decode(blob: &'a [u8]) -> Result<Self, ImageError> {
        if blob.len() < HEADER_SIZE || &blob[..8] != MAGIC {
            return Err(ImageError::InvalidImage);
        }
        let size = word(&blob[8..12]) as usize;
        let count = word(&blob[12..16]) as usize;
        if size > MAX_IMAGE_SIZE {
            return Err(ImageError::TooLarge);
        }
        if size == 0 || !size.is_multiple_of(SECTOR_SIZE) || count > size / SECTOR_SIZE {
            return Err(ImageError::InvalidImage);
        }
        let expected = count
            .checked_mul(RECORD_SIZE)
            .and_then(|len| len.checked_add(HEADER_SIZE))
            .ok_or(ImageError::InvalidImage)?;
        if expected != blob.len() {
            return Err(ImageError::InvalidImage);
        }
        let records = &blob[HEADER_SIZE..];
        let mut previous = None;
        for record in records.as_chunks::<RECORD_SIZE>().0 {
            let sector = word(record) as usize;
            if sector >= size / SECTOR_SIZE || previous.is_some_and(|last| sector <= last) {
                return Err(ImageError::InvalidImage);
            }
            previous = Some(sector);
        }
        Ok(Self { records, size })
    }

    /// Logical disk size, including omitted zero sectors.
    pub fn byte_len(&self) -> usize {
        self.size
    }
}

impl BlockDevice for ReadOnlyDisk<'_> {
    type Error = DiskError;
    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }
    fn sector_count(&self) -> u64 {
        (self.size / SECTOR_SIZE) as u64
    }
    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), DiskError> {
        if out.len() != SECTOR_SIZE {
            return Err(DiskError::InvalidBufferLength);
        }
        if lba >= self.sector_count() {
            return Err(DiskError::OutOfBounds);
        }
        let mut left = 0;
        let mut right = self.records.len() / RECORD_SIZE;
        while left < right {
            let middle = left + (right - left) / 2;
            let record = &self.records[middle * RECORD_SIZE..(middle + 1) * RECORD_SIZE];
            match u64::from(word(record)).cmp(&lba) {
                core::cmp::Ordering::Less => left = middle + 1,
                core::cmp::Ordering::Greater => right = middle,
                core::cmp::Ordering::Equal => {
                    out.copy_from_slice(&record[4..]);
                    return Ok(());
                }
            }
        }
        out.fill(0);
        Ok(())
    }
    fn write_sector(&mut self, _lba: u64, _data: &[u8]) -> Result<(), DiskError> {
        Err(DiskError::ReadOnly)
    }
    fn flush(&mut self) -> Result<(), DiskError> {
        Ok(())
    }
}

/// Build a deterministic sparse MBR/exFAT image containing root-level init.elf.
///
/// Logical size grows by powers of two from 512 KiB, reserving at least 256 KiB
/// for metadata/free space, up to [`MAX_IMAGE_SIZE`]. Only nonzero sectors are
/// stored; the encoder adds a four-byte sector number per stored sector.
///
/// # Errors
///
/// Returns [`ImageError`] for oversized input, host reservation failure, or
/// filesystem construction errors. No physical disk is accessed.
#[cfg(feature = "build")]
pub fn build(init: &[u8]) -> Result<Vec<u8>, ImageError> {
    use exfat_embedded::{FileSystem, Scratch};
    let size = init
        .len()
        .checked_add(256 * 1024)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(ImageError::TooLarge)?
        .max(512 * 1024);
    if size > MAX_IMAGE_SIZE {
        return Err(ImageError::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| ImageError::AllocationFailed)?;
    bytes.resize(size, 0);
    {
        let mut device = WritableDisk { bytes: &mut bytes };
        let mut sector = [0; SECTOR_SIZE];
        let mut scratch = Scratch::new(&mut sector);
        exfat_embedded::format_exfat(&mut device, &mut scratch)
            .map_err(|_| ImageError::Filesystem)?;
        let mut filesystem =
            FileSystem::mount(device, &mut scratch).map_err(|_| ImageError::Filesystem)?;
        let mut file = filesystem
            .create("init.elf", &mut scratch)
            .map_err(|_| ImageError::Filesystem)?;
        filesystem
            .append(&mut file, init, &mut scratch)
            .map_err(|_| ImageError::Filesystem)?;
        filesystem
            .flush(&mut scratch)
            .map_err(|_| ImageError::Filesystem)?;
    }
    let count = bytes
        .as_chunks::<SECTOR_SIZE>()
        .0
        .iter()
        .filter(|sector| sector.iter().any(|&byte| byte != 0))
        .count();
    let mut blob = Vec::new();
    blob.try_reserve_exact(HEADER_SIZE + count * RECORD_SIZE)
        .map_err(|_| ImageError::AllocationFailed)?;
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&(size as u32).to_le_bytes());
    blob.extend_from_slice(&(count as u32).to_le_bytes());
    for (sector, data) in bytes.as_chunks::<SECTOR_SIZE>().0.iter().enumerate() {
        if data.iter().all(|&byte| byte == 0) {
            continue;
        }
        blob.extend_from_slice(&(sector as u32).to_le_bytes());
        blob.extend_from_slice(data);
    }
    Ok(blob)
}

#[cfg(feature = "build")]
struct WritableDisk<'a> {
    bytes: &'a mut [u8],
}

#[cfg(feature = "build")]
impl BlockDevice for WritableDisk<'_> {
    type Error = DiskError;
    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }
    fn sector_count(&self) -> u64 {
        (self.bytes.len() / SECTOR_SIZE) as u64
    }
    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), DiskError> {
        if out.len() != SECTOR_SIZE {
            return Err(DiskError::InvalidBufferLength);
        }
        let start = usize::try_from(lba)
            .ok()
            .and_then(|n| n.checked_mul(SECTOR_SIZE))
            .ok_or(DiskError::OutOfBounds)?;
        out.copy_from_slice(
            self.bytes
                .get(start..start.saturating_add(SECTOR_SIZE))
                .ok_or(DiskError::OutOfBounds)?,
        );
        Ok(())
    }
    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), DiskError> {
        if data.len() != SECTOR_SIZE {
            return Err(DiskError::InvalidBufferLength);
        }
        let start = usize::try_from(lba)
            .ok()
            .and_then(|n| n.checked_mul(SECTOR_SIZE))
            .ok_or(DiskError::OutOfBounds)?;
        self.bytes
            .get_mut(start..start.saturating_add(SECTOR_SIZE))
            .ok_or(DiskError::OutOfBounds)?
            .copy_from_slice(data);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), DiskError> {
        Ok(())
    }
}

#[cfg(all(test, feature = "build"))]
mod tests {
    use super::*;
    use exfat_embedded::{FileSystem, Scratch};

    #[test]
    fn sparse_image_mounts_read_only_and_preserves_the_file() {
        let payload = b"a deterministic program fixture";
        let blob = build(payload).unwrap();
        assert_eq!(blob, build(payload).unwrap());
        let mut disk = ReadOnlyDisk::decode(&blob).unwrap();
        assert!(blob.len() < disk.byte_len() / 2);
        let mut sector = [0; SECTOR_SIZE];
        assert_eq!(disk.write_sector(0, &sector), Err(DiskError::ReadOnly));
        assert_eq!(
            disk.read_sector(u64::MAX, &mut sector),
            Err(DiskError::OutOfBounds)
        );
        assert_eq!(
            disk.read_sector(disk.sector_count(), &mut sector),
            Err(DiskError::OutOfBounds)
        );
        assert_eq!(
            disk.read_sector(0, &mut []),
            Err(DiskError::InvalidBufferLength)
        );
        disk.read_sector(disk.sector_count() - 1, &mut sector)
            .unwrap();
        assert!(sector.iter().all(|&byte| byte == 0));
        let mut scratch = Scratch::new(&mut sector);
        let mut fs = FileSystem::mount(disk, &mut scratch).unwrap();
        let mut file = fs.open("init.elf", &mut scratch).unwrap();
        let mut result = [0; 31];
        let count = fs.read(&mut file, &mut result, &mut scratch).unwrap();
        assert_eq!(&result[..count], payload);
    }

    #[test]
    fn rejects_truncated_trailing_duplicate_unsorted_and_out_of_bounds_records() {
        let blob = build(b"fixture").unwrap();
        for len in [0, 8, 15, 16, blob.len() - 1, blob.len() - RECORD_SIZE] {
            assert!(ReadOnlyDisk::decode(&blob[..len]).is_err());
        }
        let mut trailing = blob.clone();
        trailing.push(0);
        assert!(ReadOnlyDisk::decode(&trailing).is_err());
        for size in [0, 513, MAX_IMAGE_SIZE + 512] {
            let mut header = blob.clone();
            header[8..12].copy_from_slice(&(size as u32).to_le_bytes());
            assert!(ReadOnlyDisk::decode(&header).is_err());
        }
        let mut duplicate = blob.clone();
        duplicate[HEADER_SIZE + RECORD_SIZE..HEADER_SIZE + RECORD_SIZE + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(ReadOnlyDisk::decode(&duplicate).is_err());
        let mut outside = blob;
        let mut unsorted = outside.clone();
        let records = &mut unsorted[HEADER_SIZE..HEADER_SIZE + 2 * RECORD_SIZE];
        let (first, second) = records.split_at_mut(RECORD_SIZE);
        first.swap_with_slice(second);
        assert!(ReadOnlyDisk::decode(&unsorted).is_err());
        outside[HEADER_SIZE..HEADER_SIZE + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(ReadOnlyDisk::decode(&outside).is_err());
    }

    #[test]
    fn empty_capacity_requires_only_a_header() {
        let mut blob = [0_u8; HEADER_SIZE];
        blob[..8].copy_from_slice(MAGIC);
        blob[8..12].copy_from_slice(&(MAX_IMAGE_SIZE as u32).to_le_bytes());
        let mut disk = ReadOnlyDisk::decode(&blob).unwrap();
        assert_eq!(disk.byte_len(), MAX_IMAGE_SIZE);
        let mut sector = [0xff; SECTOR_SIZE];
        disk.read_sector(disk.sector_count() - 1, &mut sector)
            .unwrap();
        assert!(sector.iter().all(|&byte| byte == 0));
    }
}
