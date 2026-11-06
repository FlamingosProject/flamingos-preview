//! Owned, page-aligned backing for ELF images and guarded process stacks.
//!
//! Segments sharing a page are rejected rather than combining permissions.
//! Prefix and suffix padding are zeroed, just like ELF BSS. No raw memory is
//! accessed and no translation tables are activated by this module.

use crate::{MAX_IMAGE_SIZE, Permissions, ProgramImage};
use alloc::vec::Vec;

/// Page size used by the kernel's AArch64 translation regime.
pub const PAGE_SIZE: usize = 64 * 1024;

/// Failure to prepare page backing for a process image or stack.
#[derive(Debug, Eq, PartialEq)]
pub enum PrepareError {
    /// A segment or stack has an empty, unaligned, overflowing, or reserved range.
    InvalidRange,
    /// Two segments need the same virtual page, even if their bytes do not overlap.
    SharedPage,
    /// This translation regime cannot express a segment without read permission.
    UnsupportedPermissions,
    /// The entry does not identify a complete aligned executable instruction.
    InvalidEntry,
    /// Rounded image or stack backing exceeds its limit or overflows.
    TooLarge,
    /// The allocator could not reserve metadata or page backing.
    AllocationFailed,
}

/// One zero-initialized, physically mappable page of owned storage.
#[repr(align(65536))]
pub struct Page {
    bytes: [u8; PAGE_SIZE],
}

impl Page {
    /// Borrow the page bytes through the kernel's existing heap mapping.
    pub fn bytes(&self) -> &[u8; PAGE_SIZE] {
        &self.bytes
    }
}

fn zeroed_pages(count: usize) -> Result<Vec<Page>, PrepareError> {
    let mut pages = Vec::new();
    pages
        .try_reserve_exact(count)
        .map_err(|_| PrepareError::AllocationFailed)?;
    pages.resize_with(count, || Page {
        bytes: [0; PAGE_SIZE],
    });
    Ok(pages)
}

/// Zeroed backing for a descending stack with an unbacked guard page below it.
///
/// This owns only the stack's pages. Mapping policy must keep the guard page
/// unmapped and exclude executable segments from the guard/stack range.
pub struct StackPages {
    top: u64,
    pages: Vec<Page>,
}

impl StackPages {
    /// Allocate `count` pages ending just below the page-aligned `top` address.
    ///
    /// `top` is the exclusive end and initial stack pointer: the first push
    /// uses memory below it. A guard page must fit below the backed range.
    ///
    /// # Errors
    ///
    /// Returns [`PrepareError`] for zero pages, invalid alignment or arithmetic,
    /// backing above the image-size limit, or allocation failure.
    pub fn new(top: u64, count: usize) -> Result<Self, PrepareError> {
        if count == 0 || !top.is_multiple_of(PAGE_SIZE as u64) {
            return Err(PrepareError::InvalidRange);
        }
        let bytes = count.checked_mul(PAGE_SIZE).ok_or(PrepareError::TooLarge)?;
        if bytes > MAX_IMAGE_SIZE {
            return Err(PrepareError::TooLarge);
        }
        top.checked_sub(bytes as u64)
            .and_then(|start| start.checked_sub(PAGE_SIZE as u64))
            .ok_or(PrepareError::InvalidRange)?;
        Ok(Self {
            top,
            pages: zeroed_pages(count)?,
        })
    }

    /// Lowest backed stack address, inclusive.
    pub fn virtual_start(&self) -> u64 {
        self.top - (self.pages.len() * PAGE_SIZE) as u64
    }

    /// Initial SP and exclusive end of the stack, aligned to both 16 bytes and a page.
    pub fn top(&self) -> u64 {
        self.top
    }

    /// Start of the unbacked guard page immediately below the stack.
    pub fn guard_start(&self) -> u64 {
        self.virtual_start() - PAGE_SIZE as u64
    }

    /// Aligned, zero-initialized stack backing in increasing virtual-address order.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
}

/// Page backing for one segment, with its final virtual range and permissions.
pub struct SegmentPages {
    virtual_start: u64,
    permissions: Permissions,
    pages: Vec<Page>,
}

impl SegmentPages {
    /// Virtual address of the first page, rounded down from ELF `p_vaddr`.
    pub fn virtual_start(&self) -> u64 {
        self.virtual_start
    }

    /// Permissions to apply to every page of this segment.
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    /// Owned, aligned backing pages in increasing virtual-address order.
    ///
    /// Physical contiguity is not assumed; translate each page separately.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
}

/// Process memory backing that remains independent of any active address space.
///
/// Segment bytes are copied from the consumed image. Dropping this object
/// releases all its pages, so it must outlive any future installed mappings.
pub struct PreparedImage {
    entry: u64,
    segments: Vec<SegmentPages>,
}

impl PreparedImage {
    /// Allocate aligned backing for the image below `address_end` (exclusive).
    ///
    /// The null page is reserved. Whole pages must fit below `address_end`,
    /// which must be page aligned. Segment order is normalized and every range
    /// is checked again because callers can modify `ProgramImage` fields.
    /// Readable and executable/writable combinations are preserved as requested;
    /// this stage does not impose a write-xor-execute policy.
    ///
    /// # Errors
    ///
    /// Returns [`PrepareError`] for invalid ranges or entry, segments sharing
    /// a page, unsupported permissions, excessive backing, or allocation failure.
    pub fn new(mut image: ProgramImage, address_end: u64) -> Result<Self, PrepareError> {
        if address_end == 0 || !address_end.is_multiple_of(PAGE_SIZE as u64) {
            return Err(PrepareError::InvalidRange);
        }
        image
            .segments
            .sort_unstable_by_key(|segment| segment.virtual_address);
        let mut previous_end = 0;
        let mut total = 0_usize;
        let mut valid_entry = false;
        for segment in &image.segments {
            let start = segment.virtual_address & !(PAGE_SIZE as u64 - 1);
            let end = segment
                .virtual_address
                .checked_add(segment.data.len() as u64)
                .and_then(|end| end.checked_add(PAGE_SIZE as u64 - 1))
                .map(|end| end & !(PAGE_SIZE as u64 - 1))
                .ok_or(PrepareError::InvalidRange)?;
            if segment.data.is_empty() || start == 0 || end > address_end {
                return Err(PrepareError::InvalidRange);
            }
            if start < previous_end {
                return Err(PrepareError::SharedPage);
            }
            if !segment.permissions.read {
                return Err(PrepareError::UnsupportedPermissions);
            }
            total = total
                .checked_add(usize::try_from(end - start).map_err(|_| PrepareError::TooLarge)?)
                .ok_or(PrepareError::TooLarge)?;
            if total > MAX_IMAGE_SIZE {
                return Err(PrepareError::TooLarge);
            }
            if segment.permissions.execute
                && image.entry.is_multiple_of(4)
                && image.entry >= segment.virtual_address
                && image
                    .entry
                    .checked_add(4)
                    .is_some_and(|end| end <= segment.virtual_address + segment.data.len() as u64)
            {
                valid_entry = true;
            }
            previous_end = end;
        }
        if !valid_entry {
            return Err(PrepareError::InvalidEntry);
        }

        let mut segments = Vec::new();
        segments
            .try_reserve_exact(image.segments.len())
            .map_err(|_| PrepareError::AllocationFailed)?;
        for segment in image.segments {
            let offset = segment.virtual_address as usize & (PAGE_SIZE - 1);
            let count = (offset + segment.data.len()).div_ceil(PAGE_SIZE);
            let mut pages = zeroed_pages(count)?;
            let mut copied = 0;
            for (index, page) in pages.iter_mut().enumerate() {
                let begin = if index == 0 { offset } else { 0 };
                let len = (PAGE_SIZE - begin).min(segment.data.len() - copied);
                page.bytes[begin..begin + len].copy_from_slice(&segment.data[copied..copied + len]);
                copied += len;
            }
            segments.push(SegmentPages {
                virtual_start: segment.virtual_address - offset as u64,
                permissions: segment.permissions,
                pages,
            });
        }
        Ok(Self {
            entry: image.entry,
            segments,
        })
    }

    /// Initial instruction's virtual address, unchanged from the ELF image.
    pub fn entry(&self) -> u64 {
        self.entry
    }

    /// Nonoverlapping page-backed segments in increasing virtual-address order.
    pub fn segments(&self) -> &[SegmentPages] {
        &self.segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LoadSegment;
    use alloc::vec;

    fn image() -> ProgramImage {
        ProgramImage {
            entry: 0x1_0100,
            segments: vec![LoadSegment {
                virtual_address: 0x1_0100,
                alignment: 1,
                permissions: Permissions {
                    read: true,
                    write: false,
                    execute: true,
                },
                data: vec![0x5a; PAGE_SIZE],
            }],
        }
    }

    #[test]
    fn backs_unaligned_multi_page_segment_with_zero_padding() {
        let memory = PreparedImage::new(image(), 1 << 30).unwrap();
        let segment = &memory.segments()[0];
        assert_eq!(memory.entry(), 0x1_0100);
        assert_eq!(segment.virtual_start(), 0x1_0000);
        assert_eq!(segment.pages().len(), 2);
        assert_eq!(segment.permissions(), image().segments[0].permissions);
        for page in segment.pages() {
            assert_eq!(page.bytes().as_ptr() as usize % PAGE_SIZE, 0);
        }
        assert!(
            segment.pages()[0].bytes()[..256]
                .iter()
                .all(|&byte| byte == 0)
        );
        assert!(
            segment.pages()[0].bytes()[256..]
                .iter()
                .all(|&byte| byte == 0x5a)
        );
        assert!(
            segment.pages()[1].bytes()[..256]
                .iter()
                .all(|&byte| byte == 0x5a)
        );
        assert!(
            segment.pages()[1].bytes()[256..]
                .iter()
                .all(|&byte| byte == 0)
        );
    }

    #[test]
    fn rejects_shared_pages_before_allocating_backing() {
        let mut source = image();
        source.segments[0].data.truncate(4);
        let mut other = image().segments.remove(0);
        other.virtual_address = 0x1_0200;
        other.data.truncate(4);
        source.segments.push(other);
        assert!(matches!(
            PreparedImage::new(source, 1 << 30),
            Err(PrepareError::SharedPage)
        ));
    }

    #[test]
    fn checks_mutable_image_ranges_permissions_entry_and_limits() {
        let mut source = image();
        source.segments[0].virtual_address = u64::MAX;
        assert!(matches!(
            PreparedImage::new(source, 1 << 30),
            Err(PrepareError::InvalidRange)
        ));
        let mut source = image();
        source.segments[0].permissions.read = false;
        assert!(matches!(
            PreparedImage::new(source, 1 << 30),
            Err(PrepareError::UnsupportedPermissions)
        ));
        let mut source = image();
        source.entry += 1;
        assert!(matches!(
            PreparedImage::new(source, 1 << 30),
            Err(PrepareError::InvalidEntry)
        ));
        assert!(matches!(
            PreparedImage::new(image(), 0x2_0000),
            Err(PrepareError::InvalidRange)
        ));
        let mut source = image();
        source.segments[0].data.resize(MAX_IMAGE_SIZE, 0);
        assert!(matches!(
            PreparedImage::new(source, 1 << 30),
            Err(PrepareError::TooLarge)
        ));
    }

    #[test]
    fn stack_is_zeroed_aligned_and_has_no_backing_for_its_guard() {
        let stack = StackPages::new(1 << 30, 4).unwrap();
        assert_eq!(stack.top(), 1 << 30);
        assert_eq!(stack.top() % 16, 0);
        assert_eq!(stack.virtual_start(), (1 << 30) - 4 * PAGE_SIZE as u64);
        assert_eq!(stack.guard_start(), (1 << 30) - 5 * PAGE_SIZE as u64);
        assert_eq!(stack.pages().len(), 4);
        for page in stack.pages() {
            assert_eq!(page.bytes().as_ptr() as usize % PAGE_SIZE, 0);
            assert!(page.bytes().iter().all(|&byte| byte == 0));
        }
    }

    #[test]
    fn rejects_invalid_stack_layouts_and_images_in_reserved_stack_space() {
        for (top, count) in [
            (0, 1),
            (1 << 30, 0),
            ((1 << 30) + 1, 4),
            (PAGE_SIZE as u64, 1),
        ] {
            assert!(matches!(
                StackPages::new(top, count),
                Err(PrepareError::InvalidRange)
            ));
        }
        assert!(matches!(
            StackPages::new(1 << 30, usize::MAX),
            Err(PrepareError::TooLarge)
        ));
        let stack = StackPages::new(1 << 30, 4).unwrap();
        for address in [stack.guard_start(), stack.virtual_start()] {
            let mut source = image();
            source.entry = address;
            source.segments[0].virtual_address = address;
            assert!(matches!(
                PreparedImage::new(source, stack.guard_start()),
                Err(PrepareError::InvalidRange)
            ));
        }
    }
}
