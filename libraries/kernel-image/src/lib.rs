//! Borrowed views of a Raspberry Pi kernel ELF and its optional static symbols.

#![no_std]
#![forbid(unsafe_code)]

use core::{ops::Range, str};

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const SECTION_HEADER_SIZE: usize = 64;
const SYMBOL_SIZE: usize = 24;
const MAX_PROGRAM_HEADERS: usize = 32;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_TLS: u32 = 7;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const STT_FUNC: u8 = 2;
const SHN_LORESERVE: usize = 0xff00;

/// First RAM byte available for a kernel stack, above firmware park code.
pub const BOOT_STACK_START: u64 = 0x0001_0000;
/// Physical address where the Pi firmware starts the boot image.
pub const KERNEL_LOAD_START: u64 = 0x0008_0000;
/// Exclusive upper bound for kernel destinations, including zero-filled memory.
pub const KERNEL_LOAD_END: u64 = 0x0200_0000;
/// Physical address where stage 1 retains the executable's source bytes.
pub const ELF_STAGING_START: u64 = 0x0400_0000;
/// Maximum size in bytes of the retained executable.
pub const ELF_STAGING_SIZE: u64 = 0x0200_0000;

/// Validation failure in a kernel executable or its symbol metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The ELF header or program-header table is incomplete or malformed.
    InvalidElf,
    /// The executable is outside the static little-endian AArch64 ELF64 format.
    UnsupportedExecutable,
    /// A program-header type requires unsupported runtime behavior.
    UnsupportedSegment(u32),
    /// A loadable segment has invalid sizes, bounds, alignment, or flags.
    InvalidSegment,
    /// Two nonempty loadable segments overlap in physical memory.
    OverlappingSegments,
    /// The physical entry is outside complete, aligned executable file data.
    InvalidEntry,
    /// The section headers, static symbols, or linked strings are malformed.
    InvalidSymbols,
}

/// The borrowed bytes and destination of a nonempty loadable ELF segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadSegment<'a> {
    /// Physical address of the first byte (`p_paddr`).
    pub physical_address: u64,
    /// Runtime virtual address of the first byte (`p_vaddr`).
    pub virtual_address: u64,
    /// Memory size in bytes, including the zero-filled tail (`p_memsz`).
    pub memory_size: u64,
    /// ELF segment flags (`p_flags`).
    pub flags: u32,
    /// Bytes to copy before clearing the rest of the segment.
    pub file_bytes: &'a [u8],
}

impl LoadSegment<'_> {
    /// Whether this segment has the ELF execute flag.
    pub fn is_executable(&self) -> bool {
        self.flags & PF_X != 0
    }
}

/// A kernel ELF whose load destinations and physical entry have been checked.
#[derive(Debug)]
pub struct KernelImage<'a> {
    bytes: &'a [u8],
    header: &'a [u8; ELF_HEADER_SIZE],
    program_headers: &'a [[u8; PROGRAM_HEADER_SIZE]],
}

impl<'a> KernelImage<'a> {
    /// Validate kernel load metadata and borrow the executable.
    ///
    /// Load destinations must lie within the physical kernel window. The
    /// linker-defined, fileless boot stack may occupy `BOOT_STACK_START..KERNEL_LOAD_START`.
    /// The entry is physical and must identify a complete AArch64 instruction
    /// in a file-backed executable load. Section metadata is optional.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] for malformed or unsupported load metadata.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let header = elf_header(bytes)?;
        let count = usize::from(u16_at(header, 56));
        let offset = u64_at(header, 32);
        if usize::from(u16_at(header, 54)) != PROGRAM_HEADER_SIZE
            || !(1..=MAX_PROGRAM_HEADERS).contains(&count)
            || offset < ELF_HEADER_SIZE as u64
        {
            return Err(Error::InvalidElf);
        }
        let program_headers = file_range(bytes, offset, (count * PROGRAM_HEADER_SIZE) as u64)
            .ok_or(Error::InvalidElf)?
            .as_chunks::<PROGRAM_HEADER_SIZE>()
            .0;

        let entry = u64_at(header, 24);
        let mut valid_entry = false;
        for (index, record) in program_headers.iter().enumerate() {
            let kind = u32_at(record, 0);
            match kind {
                PT_DYNAMIC | PT_INTERP | PT_TLS => return Err(Error::UnsupportedSegment(kind)),
                PT_GNU_STACK if u32_at(record, 4) & PF_X != 0 => {
                    return Err(Error::UnsupportedSegment(kind));
                }
                PT_LOAD => {}
                _ => continue,
            }

            let load = ProgramHeader::load(record);
            let range = load.validate(bytes)?;
            if range.is_empty() {
                continue;
            }
            for previous in program_headers[..index].iter().filter_map(load_header) {
                if previous.memory_size != 0
                    && range.start < previous.physical_address + previous.memory_size
                    && previous.physical_address < range.end
                {
                    return Err(Error::OverlappingSegments);
                }
            }
            valid_entry |= load.flags & PF_X != 0
                && entry.is_multiple_of(4)
                && entry >= range.start
                && entry
                    .checked_add(4)
                    .is_some_and(|end| end <= range.start + load.file_size);
        }
        if !valid_entry {
            return Err(Error::InvalidEntry);
        }
        Ok(Self {
            bytes,
            header,
            program_headers,
        })
    }

    /// Validated physical entry address (`e_entry`).
    pub fn entry(&self) -> u64 {
        u64_at(self.header, 24)
    }

    /// Nonempty loadable segments in program-header order.
    pub fn segments(&self) -> impl Iterator<Item = LoadSegment<'a>> + '_ {
        self.program_headers
            .iter()
            .filter_map(load_header)
            .filter(|load| load.memory_size != 0)
            .map(|load| LoadSegment {
                physical_address: load.physical_address,
                virtual_address: load.virtual_address,
                memory_size: load.memory_size,
                flags: load.flags,
                file_bytes: file_range(self.bytes, load.file_offset, load.file_size)
                    .expect("validated segment file range"),
            })
    }

    /// Validate the ordinary static symbol table and its linked string table.
    ///
    /// Returns `None` when the ELF contains no `SHT_SYMTAB` section.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSymbols`] for malformed section or symbol data.
    pub fn symbols(&self) -> Result<Option<SymbolTable<'a>>, Error> {
        SymbolTable::from_header(self.bytes, self.header)
    }
}

struct ProgramHeader {
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

impl ProgramHeader {
    fn load(record: &[u8; PROGRAM_HEADER_SIZE]) -> Self {
        Self {
            flags: u32_at(record, 4),
            file_offset: u64_at(record, 8),
            virtual_address: u64_at(record, 16),
            physical_address: u64_at(record, 24),
            file_size: u64_at(record, 32),
            memory_size: u64_at(record, 40),
            alignment: u64_at(record, 48),
        }
    }

    fn validate(&self, bytes: &[u8]) -> Result<Range<u64>, Error> {
        let start = self.physical_address;
        let end = start
            .checked_add(self.memory_size)
            .ok_or(Error::InvalidSegment)?;
        if self.file_size > self.memory_size
            || file_range(bytes, self.file_offset, self.file_size).is_none()
            || self.virtual_address.checked_add(self.memory_size).is_none()
            || self.flags & !(PF_R | PF_W | PF_X) != 0
            || (self.alignment > 1
                && (!self.alignment.is_power_of_two()
                    || self.virtual_address % self.alignment != self.file_offset % self.alignment))
        {
            return Err(Error::InvalidSegment);
        }

        let boot_stack = start == BOOT_STACK_START
            && end == KERNEL_LOAD_START
            && self.file_size == 0
            && self.flags == (PF_R | PF_W);
        if self.memory_size != 0
            && !boot_stack
            && (start < KERNEL_LOAD_START || end > KERNEL_LOAD_END)
        {
            return Err(Error::InvalidSegment);
        }
        Ok(start..end)
    }
}

fn load_header(record: &[u8; PROGRAM_HEADER_SIZE]) -> Option<ProgramHeader> {
    (u32_at(record, 0) == PT_LOAD).then(|| ProgramHeader::load(record))
}

/// A named function from the kernel's static ELF symbol table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Symbol<'a> {
    /// Name as stored in the ELF, including any Rust mangling.
    pub name: &'a str,
    /// First byte of the function at its linked virtual address.
    pub address: u64,
    /// Function size in bytes.
    pub size: u64,
}

impl<'a> Symbol<'a> {
    /// The linked virtual address of the first byte.
    pub fn addr(&self) -> u64 {
        self.address
    }

    /// The function's byte size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The name as stored in the ELF string table, including any Rust mangling.
    pub fn name(&self) -> &'a str {
        self.name
    }
}

/// An allocation-free view of the kernel's static symbols and their names.
#[derive(Debug)]
pub struct SymbolTable<'a> {
    entries: &'a [[u8; SYMBOL_SIZE]],
    strings: &'a [u8],
}

impl<'a> SymbolTable<'a> {
    /// Borrow the ordinary static symbols from a kernel ELF.
    ///
    /// Returns `None` if there is no static symbol table. Only the ELF header
    /// and the section metadata needed to read static symbols are checked.
    ///
    /// # Errors
    /// Returns [`Error`] for malformed ELF headers or symbol metadata.
    pub fn parse(bytes: &'a [u8]) -> Result<Option<Self>, Error> {
        Self::from_header(bytes, elf_header(bytes)?)
    }

    fn from_header(bytes: &'a [u8], header: &[u8; ELF_HEADER_SIZE]) -> Result<Option<Self>, Error> {
        let offset = u64_at(header, 40);
        let count = usize::from(u16_at(header, 60));
        if offset == 0 && count == 0 {
            return Ok(None);
        }
        if offset < ELF_HEADER_SIZE as u64
            || !(1..SHN_LORESERVE).contains(&count)
            || usize::from(u16_at(header, 58)) != SECTION_HEADER_SIZE
        {
            return Err(Error::InvalidSymbols);
        }
        let sections = file_range(bytes, offset, (count * SECTION_HEADER_SIZE) as u64)
            .ok_or(Error::InvalidSymbols)?
            .as_chunks::<SECTION_HEADER_SIZE>()
            .0;
        let mut tables = sections
            .iter()
            .filter(|section| u32_at(*section, 4) == SHT_SYMTAB);
        let Some(table) = tables.next() else {
            return Ok(None);
        };
        if tables.next().is_some() || u64_at(table, 56) != SYMBOL_SIZE as u64 {
            return Err(Error::InvalidSymbols);
        }

        let (entries, trailing) = section_data(bytes, table)?.as_chunks::<SYMBOL_SIZE>();
        if !trailing.is_empty() || entries.first() != Some(&[0; SYMBOL_SIZE]) {
            return Err(Error::InvalidSymbols);
        }
        let string_index = usize::try_from(u32_at(table, 40)).map_err(|_| Error::InvalidSymbols)?;
        let strings = sections.get(string_index).ok_or(Error::InvalidSymbols)?;
        if u32_at(strings, 4) != SHT_STRTAB {
            return Err(Error::InvalidSymbols);
        }
        let strings = section_data(bytes, strings)?;
        if strings.first() != Some(&0) || strings.last() != Some(&0) {
            return Err(Error::InvalidSymbols);
        }
        for entry in entries {
            let name_offset =
                usize::try_from(u32_at(entry, 0)).map_err(|_| Error::InvalidSymbols)?;
            let section_index = usize::from(u16_at(entry, 6));
            if name_offset >= strings.len()
                || (section_index < SHN_LORESERVE && section_index >= count)
            {
                return Err(Error::InvalidSymbols);
            }
        }
        Ok(Some(Self { entries, strings }))
    }

    /// Find a defined, named function containing this virtual address.
    ///
    /// ELF symbol order is unrestricted. For overlapping functions, prefer
    /// the closest start address, then the narrowest range at that address.
    pub fn lookup(&self, address: u64) -> Option<Symbol<'a>> {
        let mut best: Option<Symbol<'a>> = None;
        for entry in self.entries {
            let section_index = usize::from(u16_at(entry, 6));
            let value = u64_at(entry, 8);
            let size = u64_at(entry, 16);
            if entry[4] & 0xf != STT_FUNC
                || !(1..SHN_LORESERVE).contains(&section_index)
                || value == 0
                || value.checked_add(size).is_none_or(|end| address >= end)
                || address < value
                || best.is_some_and(|symbol| {
                    value < symbol.address || (value == symbol.address && size >= symbol.size)
                })
            {
                continue;
            }
            let offset = u32_at(entry, 0) as usize;
            let name = self.strings[offset..].split(|&byte| byte == 0).next()?;
            if let Ok(name) = str::from_utf8(name)
                && !name.is_empty()
            {
                best = Some(Symbol {
                    name,
                    address: value,
                    size,
                });
            }
        }
        best
    }
}

fn elf_header(bytes: &[u8]) -> Result<&[u8; ELF_HEADER_SIZE], Error> {
    let (header, _) = bytes
        .split_first_chunk::<ELF_HEADER_SIZE>()
        .ok_or(Error::InvalidElf)?;
    if &header[..4] != b"\x7fELF" || usize::from(u16_at(header, 52)) != ELF_HEADER_SIZE {
        return Err(Error::InvalidElf);
    }
    if header[4..7] != [2, 1, 1]
        || u16_at(header, 16) != 2 // ET_EXEC
        || u16_at(header, 18) != 183 // EM_AARCH64
        || u32_at(header, 20) != 1
        || u32_at(header, 48) != 0
    {
        return Err(Error::UnsupportedExecutable);
    }
    Ok(header)
}

fn section_data<'a>(
    bytes: &'a [u8],
    section: &[u8; SECTION_HEADER_SIZE],
) -> Result<&'a [u8], Error> {
    file_range(bytes, u64_at(section, 24), u64_at(section, 32)).ok_or(Error::InvalidSymbols)
}

fn file_range(bytes: &[u8], offset: u64, size: u64) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let end = usize::try_from(offset.checked_add(size)?).ok()?;
    bytes.get(start..end)
}

fn u16_at(record: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([record[offset], record[offset + 1]])
}

fn u32_at(record: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
}

fn u64_at(record: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(record[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: u64 = 0xffff_ffff_c000_0000;
    const SECTION_TABLE: usize = 0x3000;
    const SYMBOLS: usize = 0x3100;
    const STRINGS: usize = 0x3200;
    const NAMES: &[u8] = b"\0later\0early\0narrow\0object\0undefined\0empty\0";

    fn write<const N: usize>(bytes: &mut [u8], offset: usize, value: [u8; N]) {
        bytes[offset..offset + N].copy_from_slice(&value);
    }

    fn program_header(index: usize) -> usize {
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * index
    }

    fn section_header(index: usize) -> usize {
        SECTION_TABLE + SECTION_HEADER_SIZE * index
    }

    fn fixture() -> [u8; STRINGS + NAMES.len()] {
        let mut elf = [0; STRINGS + NAMES.len()];
        write(&mut elf, 0, *b"\x7fELF\x02\x01\x01");
        write(&mut elf, 16, 2_u16.to_le_bytes());
        write(&mut elf, 18, 183_u16.to_le_bytes());
        write(&mut elf, 20, 1_u32.to_le_bytes());
        write(&mut elf, 24, KERNEL_LOAD_START.to_le_bytes());
        write(&mut elf, 32, (ELF_HEADER_SIZE as u64).to_le_bytes());
        write(&mut elf, 52, (ELF_HEADER_SIZE as u16).to_le_bytes());
        write(&mut elf, 54, (PROGRAM_HEADER_SIZE as u16).to_le_bytes());
        write(&mut elf, 56, 4_u16.to_le_bytes());

        // Code, data and BSS, the fileless heap, and the low boot stack.
        let segments = [
            (PF_R | PF_X, [0x1000, CODE, KERNEL_LOAD_START, 0x100, 0x100]),
            (PF_R | PF_W, [0x2000, CODE + 0x1000, 0x81000, 8, 0x100]),
            (PF_R | PF_W, [0x3000, CODE + 0x2000, 0x82000, 0, 0x1000000]),
            (
                PF_R | PF_W,
                [
                    0x3000,
                    CODE + 0x1890000,
                    BOOT_STACK_START,
                    0,
                    KERNEL_LOAD_START - BOOT_STACK_START,
                ],
            ),
        ];
        for (index, (flags, fields)) in segments.into_iter().enumerate() {
            let header = program_header(index);
            write(&mut elf, header, PT_LOAD.to_le_bytes());
            write(&mut elf, header + 4, flags.to_le_bytes());
            for (field, value) in fields.into_iter().chain([0x1000]).enumerate() {
                write(&mut elf, header + 8 + field * 8, value.to_le_bytes());
            }
        }
        write(&mut elf, 0x1000, *b"code");
        write(&mut elf, 0x2000, *b"dataabcd");
        elf
    }

    fn with_symbols(elf: &mut [u8]) {
        write(elf, 40, (SECTION_TABLE as u64).to_le_bytes());
        write(elf, 58, (SECTION_HEADER_SIZE as u16).to_le_bytes());
        write(elf, 60, 4_u16.to_le_bytes());
        write(elf, section_header(1) + 4, 1_u32.to_le_bytes()); // SHT_PROGBITS
        write(elf, section_header(2) + 4, SHT_SYMTAB.to_le_bytes());
        write(elf, section_header(2) + 24, (SYMBOLS as u64).to_le_bytes());
        write(
            elf,
            section_header(2) + 32,
            (7 * SYMBOL_SIZE as u64).to_le_bytes(),
        );
        write(elf, section_header(2) + 40, 3_u32.to_le_bytes());
        write(
            elf,
            section_header(2) + 56,
            (SYMBOL_SIZE as u64).to_le_bytes(),
        );
        write(elf, section_header(3) + 4, SHT_STRTAB.to_le_bytes());
        write(elf, section_header(3) + 24, (STRINGS as u64).to_le_bytes());
        write(
            elf,
            section_header(3) + 32,
            (NAMES.len() as u64).to_le_bytes(),
        );
        elf[STRINGS..].copy_from_slice(NAMES);

        let symbols = [
            (1_u32, STT_FUNC, 1_u16, CODE + 0x80, 0x10_u64),
            (7, STT_FUNC, 1, CODE, 0x40),
            (13, STT_FUNC, 1, CODE, 0x10),
            (20, 1, 1, CODE + 4, 4), // STT_OBJECT
            (27, STT_FUNC, 0, CODE + 4, 4),
            (37, STT_FUNC, 1, CODE + 4, 0),
        ];
        for (index, (name, kind, section, address, size)) in symbols.into_iter().enumerate() {
            let offset = SYMBOLS + (index + 1) * SYMBOL_SIZE;
            write(elf, offset, name.to_le_bytes());
            elf[offset + 4] = kind;
            write(elf, offset + 6, section.to_le_bytes());
            write(elf, offset + 8, address.to_le_bytes());
            write(elf, offset + 16, size.to_le_bytes());
        }
    }

    #[test]
    fn loads_the_linker_layout_without_sections() {
        let elf = fixture();
        let image = KernelImage::parse(&elf).unwrap();
        assert_eq!(image.entry(), KERNEL_LOAD_START);
        assert!(image.symbols().unwrap().is_none());
        let mut loads = image.segments();
        let code = loads.next().unwrap();
        assert!(code.is_executable());
        assert_eq!(code.virtual_address, CODE);
        assert_eq!(code.physical_address, KERNEL_LOAD_START);
        assert_eq!(&code.file_bytes[..4], b"code");
        let data = loads.next().unwrap();
        assert_eq!(data.file_bytes, b"dataabcd");
        assert_eq!(data.memory_size, 0x100);
        let heap = loads.next().unwrap();
        assert!(heap.file_bytes.is_empty());
        assert_eq!(heap.memory_size, 16 * 1024 * 1024);
        let stack = loads.next().unwrap();
        assert!(stack.file_bytes.is_empty());
        assert_eq!(stack.physical_address, BOOT_STACK_START);
        assert_eq!(stack.memory_size, KERNEL_LOAD_START - BOOT_STACK_START);
        assert!(loads.next().is_none());
    }

    #[test]
    fn rejects_incomplete_headers_and_program_table() {
        let mut elf = fixture();
        for len in [0, ELF_HEADER_SIZE - 1, program_header(4) - 1] {
            assert_eq!(
                KernelImage::parse(&elf[..len]).unwrap_err(),
                Error::InvalidElf
            );
        }
        write(&mut elf, 32, u64::MAX.to_le_bytes());
        assert_eq!(KernelImage::parse(&elf).unwrap_err(), Error::InvalidElf);
        elf[4] = 1;
        assert_eq!(
            KernelImage::parse(&elf).unwrap_err(),
            Error::UnsupportedExecutable
        );
    }

    #[test]
    fn rejects_invalid_load_destinations_and_sizes() {
        for (offset, invalid) in [
            (program_header(0) + 8, u64::MAX),
            (program_header(0) + 16, u64::MAX - 0x7f),
            (program_header(0) + 32, 0x101),
            (program_header(0) + 48, 3),
            (program_header(1) + 24, KERNEL_LOAD_END),
            (program_header(2) + 40, u64::MAX),
            (program_header(3) + 32, 1),
            (program_header(3) + 24, 0),
        ] {
            let mut elf = fixture();
            write(&mut elf, offset, invalid.to_le_bytes());
            assert_eq!(KernelImage::parse(&elf).unwrap_err(), Error::InvalidSegment);
        }
        let mut elf = fixture();
        write(
            &mut elf,
            program_header(1) + 24,
            KERNEL_LOAD_START.to_le_bytes(),
        );
        assert_eq!(
            KernelImage::parse(&elf).unwrap_err(),
            Error::OverlappingSegments
        );
    }

    #[test]
    fn entry_must_be_physical_aligned_and_file_backed() {
        let mut elf = fixture();
        write(&mut elf, program_header(0) + 40, 0x200_u64.to_le_bytes());
        for entry in [
            CODE,
            KERNEL_LOAD_START + 1,
            KERNEL_LOAD_START + 0x100,
            0x81000,
        ] {
            write(&mut elf, 24, entry.to_le_bytes());
            assert_eq!(KernelImage::parse(&elf).unwrap_err(), Error::InvalidEntry);
        }
        write(&mut elf, 24, (KERNEL_LOAD_START + 0xfc).to_le_bytes());
        assert!(KernelImage::parse(&elf).is_ok());
    }

    #[test]
    fn rejects_dynamic_and_interpreted_executables() {
        for kind in [PT_DYNAMIC, PT_INTERP, PT_TLS] {
            let mut elf = fixture();
            write(&mut elf, program_header(1), kind.to_le_bytes());
            assert_eq!(
                KernelImage::parse(&elf).unwrap_err(),
                Error::UnsupportedSegment(kind)
            );
        }
    }

    #[test]
    fn finds_unsorted_defined_functions_and_resolves_aliases() {
        let mut elf = fixture();
        with_symbols(&mut elf);
        let image = KernelImage::parse(&elf).unwrap();
        let table = image.symbols().unwrap().unwrap();
        let symbol = table.lookup(CODE + 4).unwrap();
        assert_eq!(symbol.name(), "narrow");
        assert_eq!(symbol.addr(), CODE);
        assert_eq!(symbol.size(), 0x10);
        assert_eq!(table.lookup(CODE + 0x10).unwrap().name(), "early");
        assert!(table.lookup(CODE + 0x40).is_none());
        assert_eq!(table.lookup(CODE + 0x80).unwrap().name(), "later");
        assert!(table.lookup(KERNEL_LOAD_START).is_none());

        write(&mut elf, 24, CODE.to_le_bytes());
        assert!(KernelImage::parse(&elf).is_err());
        assert!(SymbolTable::parse(&elf).unwrap().is_some());
    }

    #[test]
    fn rejects_invalid_symbol_metadata_independently_of_loads() {
        let mut elf = fixture();
        with_symbols(&mut elf);
        let mut overflow = elf;
        write(&mut overflow, 40, u64::MAX.to_le_bytes());
        let mut bad_size = elf;
        write(&mut bad_size, section_header(2) + 32, 7_u64.to_le_bytes());
        let mut bad_link = elf;
        write(&mut bad_link, section_header(2) + 40, 1_u32.to_le_bytes());
        let mut bad_name = elf;
        write(
            &mut bad_name,
            SYMBOLS + SYMBOL_SIZE,
            (NAMES.len() as u32).to_le_bytes(),
        );
        let mut unterminated = elf;
        *unterminated.last_mut().unwrap() = b'x';
        for invalid in [overflow, bad_size, bad_link, bad_name, unterminated] {
            let image = KernelImage::parse(&invalid).unwrap();
            assert_eq!(image.symbols().unwrap_err(), Error::InvalidSymbols);
        }
    }
}
