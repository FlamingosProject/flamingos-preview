//! Exercise malformed ELF boundaries and loading through a real in-memory exFAT mount.

use super::*;
use alloc::{rc::Rc, vec};
use core::cell::Cell;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Build an ELF64 fixture with RX code, RW initialized data, and an eight-byte BSS.
///
/// The 64-byte ELF header is followed by two 56-byte program headers at offsets
/// 64 and 120. File payloads begin at 512 and 516. Keeping the fixture encoded
/// directly makes each malformed-field test independent of a cross compiler.
fn executable() -> Vec<u8> {
    let mut bytes = vec![0; 520];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, abi::ET_EXEC);
    put16(&mut bytes, 18, abi::EM_AARCH64);
    put32(&mut bytes, 20, 1);
    put64(&mut bytes, 24, 0x1_0200);
    put64(&mut bytes, 32, 64);
    put16(&mut bytes, 52, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 2);
    put32(&mut bytes, 64, abi::PT_LOAD);
    put32(&mut bytes, 68, abi::PF_R | abi::PF_X);
    put64(&mut bytes, 72, 512);
    put64(&mut bytes, 80, 0x1_0200);
    put64(&mut bytes, 96, 4);
    put64(&mut bytes, 104, 4);
    put64(&mut bytes, 112, 0x1000);
    put32(&mut bytes, 120, abi::PT_LOAD);
    put32(&mut bytes, 124, abi::PF_R | abi::PF_W);
    put64(&mut bytes, 128, 516);
    put64(&mut bytes, 136, 0x2_0204);
    put64(&mut bytes, 152, 4);
    put64(&mut bytes, 160, 12);
    put64(&mut bytes, 168, 0x1000);
    // AArch64 `b .` provides one complete instruction at the entry address.
    bytes[512..516].copy_from_slice(&0x1400_0000_u32.to_le_bytes());
    bytes[516..].copy_from_slice(b"data");
    bytes
}

fn rejected(bytes: &[u8], error: ImageError) {
    assert_eq!(ProgramImage::from_elf(bytes).unwrap_err(), error);
}

#[test]
fn loads_owned_code_data_and_zero_filled_bss() {
    let mut bytes = executable();
    let image = ProgramImage::from_elf(&bytes).unwrap();
    bytes.fill(0xff);
    assert_eq!(image.entry, 0x1_0200);
    assert_eq!(image.segments.len(), 2);
    assert_eq!(image.segments[0].data, 0x1400_0000_u32.to_le_bytes());
    assert!(image.segments[0].permissions.execute);
    assert!(!image.segments[0].permissions.write);
    assert_eq!(image.segments[1].virtual_address, 0x2_0204);
    assert_eq!(image.segments[1].alignment, 0x1000);
    assert!(image.segments[1].permissions.write);
    assert!(!image.segments[1].permissions.execute);
    assert_eq!(image.segments[1].data, b"data\0\0\0\0\0\0\0\0");
}

#[test]
fn rejects_all_truncated_prefixes() {
    let bytes = executable();
    for len in 0..bytes.len() {
        assert!(
            ProgramImage::from_elf(&bytes[..len]).is_err(),
            "length {len}"
        );
    }
}

#[test]
fn rejects_wrong_architecture_class_endianness_and_type() {
    let mut bytes = executable();
    put16(&mut bytes, 18, abi::EM_X86_64);
    rejected(&bytes, ImageError::UnsupportedExecutable);
    let mut bytes = executable();
    bytes[4] = 1;
    rejected(&bytes, ImageError::UnsupportedExecutable);
    let mut bytes = executable();
    bytes[5] = 2;
    rejected(&bytes, ImageError::InvalidElf);
    for kind in [abi::ET_DYN, abi::ET_REL] {
        let mut bytes = executable();
        put16(&mut bytes, 16, kind);
        rejected(&bytes, ImageError::UnsupportedExecutable);
    }
}

#[test]
fn rejects_dynamic_linking_tls_and_executable_stack() {
    for kind in [
        abi::PT_INTERP,
        abi::PT_DYNAMIC,
        abi::PT_TLS,
        abi::PT_GNU_STACK,
    ] {
        let mut bytes = executable();
        put32(&mut bytes, 64, kind);
        rejected(&bytes, ImageError::UnsupportedSegment(kind));
    }
}

#[test]
fn rejects_invalid_program_header_tables() {
    for (offset, value) in [(52, 63), (54, 55), (56, 0)] {
        let mut bytes = executable();
        put16(&mut bytes, offset, value);
        rejected(&bytes, ImageError::InvalidElf);
    }
    let mut bytes = executable();
    put64(&mut bytes, 32, u64::MAX);
    rejected(&bytes, ImageError::InvalidElf);
}

#[test]
fn rejects_overflow_and_invalid_segment_ranges() {
    for (offset, value) in [
        (72, u64::MAX),
        (80, u64::MAX),
        (96, 5),
        (80, 0),
        (80, USER_ADDRESS_END),
    ] {
        let mut bytes = executable();
        put64(&mut bytes, offset, value);
        rejected(&bytes, ImageError::InvalidSegment);
    }
}

#[test]
fn rejects_alignment_and_overlapping_segments() {
    for (offset, value) in [(112, 3), (112, 0x2_0000), (80, 0x1_0201)] {
        let mut bytes = executable();
        put64(&mut bytes, offset, value);
        rejected(&bytes, ImageError::InvalidSegment);
    }
    let mut bytes = executable();
    put64(&mut bytes, 136, 0x1_0200);
    put64(&mut bytes, 168, 1);
    rejected(&bytes, ImageError::OverlappingSegments);
}

#[test]
fn requires_a_complete_aligned_instruction_in_executable_file_data() {
    for entry in [0, 0x1_0201, 0x1_0204, 0x2_0204, 0x2_0208] {
        let mut bytes = executable();
        put64(&mut bytes, 24, entry);
        rejected(&bytes, ImageError::InvalidEntry);
    }
    let mut bytes = executable();
    put32(&mut bytes, 68, abi::PF_R);
    rejected(&bytes, ImageError::InvalidEntry);
}

#[test]
fn caps_headers_file_size_and_zero_fill_size() {
    let mut bytes = executable();
    put16(&mut bytes, 56, MAX_SEGMENTS as u16 + 1);
    rejected(&bytes, ImageError::TooLarge);
    let mut bytes = executable();
    bytes.resize(MAX_FILE_SIZE + 1, 0);
    rejected(&bytes, ImageError::TooLarge);
    let mut bytes = executable();
    put64(&mut bytes, 160, MAX_IMAGE_SIZE as u64);
    rejected(&bytes, ImageError::TooLarge);
}

struct MemoryDisk {
    bytes: Vec<u8>,
    read_only: Rc<Cell<bool>>,
    fail_reads: Rc<Cell<bool>>,
}

impl BlockDevice for MemoryDisk {
    type Error = ();

    fn sector_size(&self) -> usize {
        512
    }
    fn sector_count(&self) -> u64 {
        (self.bytes.len() / 512) as u64
    }
    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), ()> {
        if self.fail_reads.get() {
            return Err(());
        }
        let start = lba as usize * 512;
        out.copy_from_slice(self.bytes.get(start..start + 512).ok_or(())?);
        Ok(())
    }
    fn write_sector(&mut self, lba: u64, bytes: &[u8]) -> Result<(), ()> {
        assert!(!self.read_only.get(), "loader attempted a disk write");
        let start = lba as usize * 512;
        self.bytes
            .get_mut(start..start + 512)
            .ok_or(())?
            .copy_from_slice(bytes);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), ()> {
        assert!(!self.read_only.get(), "loader attempted a disk flush");
        Ok(())
    }
}

#[test]
fn loads_from_exfat_read_only_and_retains_image_after_unmount() {
    let read_only = Rc::new(Cell::new(false));
    let fail_reads = Rc::new(Cell::new(false));
    let mut disk = MemoryDisk {
        bytes: vec![0; 4096 * 512],
        read_only: read_only.clone(),
        fail_reads: fail_reads.clone(),
    };
    let mut buffer = [0; 512];
    let mut scratch = Scratch::new(&mut buffer);
    exfat_embedded::format_exfat(&mut disk, &mut scratch).unwrap();
    let mut fs = FileSystem::mount(disk, &mut scratch).unwrap();
    let mut file = fs.create("init.elf", &mut scratch).unwrap();
    fs.append(&mut file, &executable(), &mut scratch).unwrap();
    read_only.set(true);
    let image = load(&mut fs, "init.elf", &mut scratch).unwrap();
    assert!(matches!(
        load(&mut fs, "missing.elf", &mut scratch),
        Err(LoadError::Filesystem(_))
    ));
    fail_reads.set(true);
    assert!(matches!(
        load(&mut fs, "init.elf", &mut scratch),
        Err(LoadError::Filesystem(exfat_embedded::Error::Device(())))
    ));
    drop(fs);
    assert_eq!(image.entry, 0x1_0200);
    assert_eq!(image.segments[1].data, b"data\0\0\0\0\0\0\0\0");
}
