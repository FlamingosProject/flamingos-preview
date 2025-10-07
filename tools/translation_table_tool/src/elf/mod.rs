use crate::memory::{AccessPermissions, AttributeFields, MemoryAttributes, MemoryRegion, Alignment, GRANULE_64KIB_SIZE};
use crate::Result;
use anyhow::anyhow;
use goblin::elf::{Elf, ProgramHeader};
use std::collections::HashMap;
use std::fmt;
use std::fs;

const SECTION_FLAG_ALLOC: u64 = 2;

pub struct KernelELF {
    elf: Elf<'static>,
    symbols: HashMap<String, u64>,
}

impl KernelELF {
    pub fn new(kernel_elf_path: &str) -> Result<Self> {
        let data = fs::read(kernel_elf_path)?;
        let elf = Elf::parse(&data)?;
        
        let mut symbols = HashMap::new();
        
        // Parse symbol table
        for sym in elf.syms.iter() {
            if let Some(name) = elf.strtab.get_at(sym.st_name) {
                symbols.insert(name.to_string(), sym.st_value);
            }
        }

        // Also check dynamic symbols if they exist
        for sym in elf.dynsyms.iter() {
            if let Some(name) = elf.dynstrtab.get_at(sym.st_name) {
                symbols.insert(name.to_string(), sym.st_value);
            }
        }

        let data_static = Box::leak(data.into_boxed_slice());
        let elf = Elf::parse(data_static)?;

        Ok(Self {
            elf,
            symbols,
        })
    }

    pub fn machine(&self) -> String {
        match self.elf.header.e_machine {
            goblin::elf::header::EM_AARCH64 => "AArch64".to_string(),
            _ => format!("Unknown({})", self.elf.header.e_machine),
        }
    }

    pub fn symbol_value(&self, symbol_name: &str) -> Result<u64> {
        self.symbols
            .get(symbol_name)
            .copied()
            .ok_or_else(|| anyhow!("Symbol '{}' not found", symbol_name))
    }

    pub fn segment_containing_virt_addr(&self, virt_addr: u64) -> Option<&ProgramHeader> {
        self.elf.program_headers.iter().find(|ph| {
            ph.p_vaddr <= virt_addr && virt_addr < ph.p_vaddr + ph.p_memsz
        })
    }

    pub fn virt_to_phys(&self, virt_addr: u64) -> Result<u64> {
        let segment = self
            .segment_containing_virt_addr(virt_addr)
            .ok_or_else(|| anyhow!("No segment contains virtual address {:#x}", virt_addr))?;
        
        let translation_offset = segment.p_vaddr - segment.p_paddr;
        Ok(virt_addr - translation_offset)
    }

    pub fn virt_addr_to_file_offset(&self, virt_addr: u64) -> Result<u64> {
        let segment = self
            .segment_containing_virt_addr(virt_addr)
            .ok_or_else(|| anyhow!("No segment contains virtual address {:#x}", virt_addr))?;
        
        let offset_in_segment = virt_addr - segment.p_vaddr;
        Ok(segment.p_offset + offset_in_segment)
    }

    pub fn sections_in_segment(&self, segment: &ProgramHeader) -> String {
        let head = segment.p_vaddr;
        let tail = segment.p_vaddr + segment.p_memsz;

        let section_names: Vec<String> = self.elf.section_headers
            .iter()
            .filter_map(|sh| {
                if sh.sh_addr >= head && sh.sh_addr < tail && (sh.sh_flags & SECTION_FLAG_ALLOC) != 0 {
                    self.elf.shdr_strtab.get_at(sh.sh_name).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        section_names.join(" ")
    }

    pub fn select_load_segments(&self) -> Vec<&ProgramHeader> {
        self.elf.program_headers
            .iter()
            .filter(|ph| ph.p_type == goblin::elf::program_header::PT_LOAD)
            .collect()
    }

    pub fn segment_get_acc_perms(&self, segment: &ProgramHeader) -> AccessPermissions {
        let readable = (segment.p_flags & goblin::elf::program_header::PF_R) != 0;
        let writable = (segment.p_flags & goblin::elf::program_header::PF_W) != 0;

        if readable && writable {
            AccessPermissions::ReadWrite
        } else if readable {
            AccessPermissions::ReadOnly
        } else {
            AccessPermissions::ReadOnly // Default to readonly for safety
        }
    }

    pub fn generate_mapping_descriptors(&self) -> Result<Vec<MappingDescriptor>> {
        let mut descriptors = Vec::new();
        let mut max_name_length = "Sections".len();

        for segment in self.select_load_segments() {
            let size = (segment.p_memsz as usize).align_up(GRANULE_64KIB_SIZE);
            let virt_start_addr = segment.p_vaddr;
            let phys_start_addr = segment.p_paddr;
            let acc_perms = self.segment_get_acc_perms(segment);
            let execute_never = (segment.p_flags & goblin::elf::program_header::PF_X) == 0;
            let section_names = self.sections_in_segment(segment);

            max_name_length = max_name_length.max(section_names.len());

            let virt_region = MemoryRegion::new(virt_start_addr, size, GRANULE_64KIB_SIZE)?;
            let phys_region = MemoryRegion::new(phys_start_addr, size, GRANULE_64KIB_SIZE)?;
            let attributes = AttributeFields::new(MemoryAttributes::CacheableDRAM, acc_perms, execute_never);

            descriptors.push(MappingDescriptor::new(
                section_names,
                virt_region,
                phys_region,
                attributes,
                max_name_length,
            ));
        }

        // Update max name length for all descriptors
        for desc in &mut descriptors {
            desc.max_section_name_length = max_name_length;
        }

        Ok(descriptors)
    }
}

pub struct MappingDescriptor {
    pub name: String,
    pub virt_region: MemoryRegion,
    pub phys_region: MemoryRegion,
    pub attributes: AttributeFields,
    pub max_section_name_length: usize,
}

impl MappingDescriptor {
    pub fn new(
        name: String,
        virt_region: MemoryRegion,
        phys_region: MemoryRegion,
        attributes: AttributeFields,
        max_section_name_length: usize,
    ) -> Self {
        Self {
            name,
            virt_region,
            phys_region,
            attributes,
            max_section_name_length,
        }
    }

    pub fn print_divider(max_section_name_length: usize) {
        print!("             ");
        for _ in 0..max_section_name_length {
            print!("-");
        }
        println!("--------------------------------------------------------------------");
    }

    pub fn print_header(max_section_name_length: usize) {
        Self::print_divider(max_section_name_length);
        print!("             ");
        print!("{:^width$}", "Sections", width = max_section_name_length);
        print!("   ");
        print!("{:^21}", "Virt Start Addr");
        print!("   ");
        print!("{:^21}", "Phys Start Addr");
        print!("   ");
        print!("{:^7}", "Size");
        print!("   ");
        println!("{:^7}", "Attr");
        Self::print_divider(max_section_name_length);
    }
}

impl fmt::Display for MappingDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = format!("{:<width$}", self.name, width = self.max_section_name_length);
        let virt_start = self.virt_region.first().unwrap_or(0).to_hex_underscore(true);
        let phys_start = self.phys_region.first().unwrap_or(0).to_hex_underscore(true);
        let size = format!("{:>3}", (self.virt_region.len() * 65536) / 1024);

        write!(
            f,
            "{} | {} | {} | {} KiB | {}",
            name, virt_start, phys_start, size, self.attributes
        )
    }
}