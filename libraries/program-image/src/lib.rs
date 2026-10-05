//! Prepare owned process images from static AArch64 ELF executables.
//!
//! [`load`] reads from an existing exFAT mount; [`ProgramImage::from_elf`]
//! accepts bytes from any source. Both produce segment buffers independent of
//! the source's lifetime. Loading does not install mappings or execute code.
//!
//! This crate uses `alloc` but neither `std` nor unsafe code. Callers must
//! initialize an allocator before loading and serialize filesystem access.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use elf::{
    abi,
    endian::LittleEndian,
    file::{Class, FileHeader, parse_ident},
    segment::SegmentTable,
};
use exfat_embedded::{BlockDevice, FileSystem, Scratch};

/// Maximum executable file size in bytes, including headers and unused sections.
pub const MAX_FILE_SIZE: usize = 4 * 1024 * 1024;
/// Maximum sum of loadable segment sizes, including zero-filled tails, in bytes.
pub const MAX_IMAGE_SIZE: usize = 8 * 1024 * 1024;
/// Maximum program-header count, including headers that do not describe loads.
pub const MAX_SEGMENTS: usize = 32;
/// Exclusive upper bound on segment addresses in the lower 48-bit address range.
///
/// Process creation must additionally check its own address-space layout.
pub const USER_ADDRESS_END: u64 = 1 << 48;

/// An executable could not be validated or its segment storage allocated.
#[derive(Debug, Eq, PartialEq)]
pub enum ImageError {
    /// The ELF identification, header, or program-header table is malformed.
    InvalidElf,
    /// The ELF class, machine, type, version, or architecture flags are unsupported.
    UnsupportedExecutable,
    /// A program-header type requires unsupported runtime services.
    ///
    /// The payload is the ELF `p_type` value. Interpreters, dynamic linking,
    /// TLS, and executable stack requests are rejected.
    UnsupportedSegment(u32),
    /// A loadable segment has invalid sizes, addresses, permissions, or alignment.
    InvalidSegment,
    /// Two nonempty loadable segments occupy overlapping virtual byte ranges.
    OverlappingSegments,
    /// The entry is not a complete aligned instruction in executable file data.
    InvalidEntry,
    /// The file size, program-header count, or total segment size exceeds a limit.
    TooLarge,
    /// The allocator could not reserve the requested buffer or segment list.
    AllocationFailed,
}

/// A filesystem read or executable preparation failed.
#[derive(Debug)]
pub enum LoadError<E> {
    /// Opening or reading the file failed, retaining the filesystem/device error.
    Filesystem(exfat_embedded::Error<E>),
    /// A read returned no bytes before reaching the file's reported length.
    UnexpectedEof,
    /// The file was too large, invalid, unsupported, or could not be allocated.
    Image(ImageError),
}

/// Access permissions requested by a loadable segment's ELF flags.
///
/// These are metadata for later process mappings; buffer access is unaffected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Permissions {
    /// The segment requests readable memory (`PF_R`).
    pub read: bool,
    /// The segment requests writable memory (`PF_W`).
    pub write: bool,
    /// The segment requests executable memory (`PF_X`).
    pub execute: bool,
}

/// Owned bytes and mapping metadata for one nonempty ELF `PT_LOAD` segment.
///
/// The buffer is an ordinary heap allocation, with no guarantee of ELF or
/// page alignment. Process creation must copy it into suitable backing memory
/// and decide how to handle segments sharing a page.
#[derive(Debug)]
pub struct LoadSegment {
    /// Virtual address at which the buffer's first byte belongs (`p_vaddr`).
    pub virtual_address: u64,
    /// ELF `p_align`; zero and one impose no alignment requirement.
    pub alignment: u64,
    /// Access permissions requested for this segment.
    pub permissions: Permissions,
    /// Exactly `p_memsz` bytes, with file data followed by a zero-filled tail.
    pub data: Vec<u8>,
}

/// A prepared executable image containing everything loaded from the ELF file.
///
/// Loader-produced images have sorted, nonoverlapping segment byte ranges and
/// a validated entry. Public fields may be changed, so consumers must preserve
/// those invariants. A runnable process still needs mappings, a stack, and an
/// initial CPU context; this image does not borrow a filesystem or file buffer.
#[derive(Debug)]
pub struct ProgramImage {
    /// Initial instruction address in executable, file-backed segment data.
    pub entry: u64,
    /// Nonempty loadable segments sorted by increasing virtual address.
    pub segments: Vec<LoadSegment>,
}

fn zeroed_bytes(len: usize) -> Result<Vec<u8>, ImageError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| ImageError::AllocationFailed)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

impl ProgramImage {
    /// Validate a static ELF64 little-endian AArch64 executable and own its data.
    ///
    /// Only `ET_EXEC` is accepted. Interpreters, dynamic linking, TLS, and
    /// executable stacks are unsupported. Section metadata is ignored.
    /// Each segment's file bytes are copied and its remaining memory is zeroed.
    /// The entry must designate a complete four-byte-aligned instruction in
    /// executable file-backed data. No mappings or process context are created.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError`] for malformed or unsupported input, overlapping
    /// segment byte ranges, an invalid entry, exceeded limits, or allocation
    /// failure. Page-level compatibility remains the process creator's policy.
    ///
    /// # Example
    ///
    /// ```
    /// use program_image::{ImageError, ProgramImage};
    ///
    /// fn prepare(file_bytes: &[u8]) -> Result<ProgramImage, ImageError> {
    ///     let image = ProgramImage::from_elf(file_bytes)?;
    ///     assert!(image.segments.iter().any(|segment| segment.permissions.execute));
    ///     Ok(image)
    /// }
    /// ```
    pub fn from_elf(bytes: &[u8]) -> Result<Self, ImageError> {
        if bytes.len() > MAX_FILE_SIZE {
            return Err(ImageError::TooLarge);
        }
        // The low-level identification parser requires a complete 16-byte input.
        let ident = parse_ident::<LittleEndian>(bytes.get(..16).ok_or(ImageError::InvalidElf)?)
            .map_err(|_| ImageError::InvalidElf)?;
        if ident.1 != Class::ELF64 {
            return Err(ImageError::UnsupportedExecutable);
        }
        // Parse only loading metadata so missing or stripped sections are irrelevant.
        let header =
            FileHeader::parse_tail(ident, bytes.get(16..64).ok_or(ImageError::InvalidElf)?)
                .map_err(|_| ImageError::InvalidElf)?;
        if header.e_machine != abi::EM_AARCH64
            || header.e_type != abi::ET_EXEC
            || header.version != 1
            || header.e_flags != 0
        {
            return Err(ImageError::UnsupportedExecutable);
        }
        if header.e_ehsize != 64 || header.e_phentsize != 56 || header.e_phnum == 0 {
            return Err(ImageError::InvalidElf);
        }
        if usize::from(header.e_phnum) > MAX_SEGMENTS {
            return Err(ImageError::TooLarge);
        }
        let table_start = usize::try_from(header.e_phoff).map_err(|_| ImageError::InvalidElf)?;
        let table_end = table_start
            .checked_add(usize::from(header.e_phnum) * 56)
            .ok_or(ImageError::InvalidElf)?;
        let table = SegmentTable::new(
            LittleEndian,
            Class::ELF64,
            bytes
                .get(table_start..table_end)
                .ok_or(ImageError::InvalidElf)?,
        );
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(table.len())
            .map_err(|_| ImageError::AllocationFailed)?;
        let mut image_size = 0_usize;
        let mut valid_entry = false;
        for index in 0..table.len() {
            // The table's iterator stops on parse errors; indexed access preserves them.
            let segment = table.get(index).map_err(|_| ImageError::InvalidElf)?;
            match segment.p_type {
                abi::PT_INTERP | abi::PT_DYNAMIC | abi::PT_TLS => {
                    return Err(ImageError::UnsupportedSegment(segment.p_type));
                }
                abi::PT_GNU_STACK if segment.p_flags & abi::PF_X != 0 => {
                    return Err(ImageError::UnsupportedSegment(segment.p_type));
                }
                abi::PT_LOAD => {}
                _ => continue,
            }
            let file_end = segment
                .p_offset
                .checked_add(segment.p_filesz)
                .ok_or(ImageError::InvalidSegment)?;
            let memory_end = segment
                .p_vaddr
                .checked_add(segment.p_memsz)
                .ok_or(ImageError::InvalidSegment)?;
            if segment.p_filesz > segment.p_memsz
                || file_end > bytes.len() as u64
                || segment.p_vaddr == 0
                || memory_end > USER_ADDRESS_END
                || segment.p_flags & !(abi::PF_R | abi::PF_W | abi::PF_X) != 0
                || (segment.p_align > 1
                    && (!segment.p_align.is_power_of_two()
                        || segment.p_vaddr % segment.p_align != segment.p_offset % segment.p_align))
            {
                return Err(ImageError::InvalidSegment);
            }
            if segment.p_memsz == 0 {
                continue;
            }
            let memory_size = usize::try_from(segment.p_memsz).map_err(|_| ImageError::TooLarge)?;
            image_size = image_size
                .checked_add(memory_size)
                .ok_or(ImageError::TooLarge)?;
            if image_size > MAX_IMAGE_SIZE {
                return Err(ImageError::TooLarge);
            }
            let permissions = Permissions {
                read: segment.p_flags & abi::PF_R != 0,
                write: segment.p_flags & abi::PF_W != 0,
                execute: segment.p_flags & abi::PF_X != 0,
            };
            if permissions.execute
                && header.e_entry % 4 == 0
                && header.e_entry >= segment.p_vaddr
                && header
                    .e_entry
                    .checked_add(4)
                    .is_some_and(|end| end <= segment.p_vaddr + segment.p_filesz)
            {
                valid_entry = true;
            }
            let mut data = zeroed_bytes(memory_size)?;
            data[..segment.p_filesz as usize]
                .copy_from_slice(&bytes[segment.p_offset as usize..file_end as usize]);
            segments.push(LoadSegment {
                virtual_address: segment.p_vaddr,
                alignment: segment.p_align,
                permissions,
                data,
            });
        }
        segments.sort_unstable_by_key(|segment| segment.virtual_address);
        for pair in segments.windows(2) {
            if pair[0].virtual_address + pair[0].data.len() as u64 > pair[1].virtual_address {
                return Err(ImageError::OverlappingSegments);
            }
        }
        if !valid_entry {
            return Err(ImageError::InvalidEntry);
        }
        Ok(Self {
            entry: header.e_entry,
            segments,
        })
    }
}

/// Read an executable from a mounted exFAT filesystem and prepare its image.
///
/// `path` uses the filesystem's path syntax; `scratch` must accommodate its
/// sector size. The filesystem and scratch storage are borrowed only during
/// this call. The returned image owns its bytes and survives unmounting.
/// This function only opens and reads; it performs no filesystem writes.
///
/// The complete file is temporarily buffered, so peak storage includes both
/// its bytes and the resulting segment buffers. File size is checked before
/// allocating, and the temporary buffer is released on success or failure.
///
/// # Errors
///
/// Returns [`LoadError`] for filesystem errors, premature end of file, or any
/// validation/allocation error from [`ProgramImage::from_elf`].
///
/// # Example
///
/// ```
/// use exfat_embedded::{BlockDevice, FileSystem, Scratch};
/// use program_image::{LoadError, ProgramImage};
///
/// fn prepare_init<D: BlockDevice>(
///     filesystem: &mut FileSystem<D>,
///     sector_buffer: &mut [u8],
/// ) -> Result<ProgramImage, LoadError<D::Error>> {
///     let mut scratch = Scratch::new(sector_buffer);
///     program_image::load(filesystem, "init.elf", &mut scratch)
/// }
/// ```
pub fn load<D: BlockDevice>(
    filesystem: &mut FileSystem<D>,
    path: &str,
    scratch: &mut Scratch<'_>,
) -> Result<ProgramImage, LoadError<D::Error>> {
    let mut file = filesystem
        .open(path, scratch)
        .map_err(LoadError::Filesystem)?;
    let len = usize::try_from(file.len()).map_err(|_| LoadError::Image(ImageError::TooLarge))?;
    if len > MAX_FILE_SIZE {
        return Err(LoadError::Image(ImageError::TooLarge));
    }
    let mut bytes = zeroed_bytes(len).map_err(LoadError::Image)?;
    let mut done = 0;
    while done < len {
        let read = filesystem
            .read(&mut file, &mut bytes[done..], scratch)
            .map_err(LoadError::Filesystem)?;
        if read == 0 {
            return Err(LoadError::UnexpectedEof);
        }
        done += read;
    }
    ProgramImage::from_elf(&bytes).map_err(LoadError::Image)
}

#[cfg(test)]
mod tests;
