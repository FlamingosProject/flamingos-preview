// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021-2023 Andre Richter <andre.o.richter@gmail.com>

use anyhow::{anyhow, Context, Result};
use goblin::elf::Elf;
use std::fs;

/// Symbol information extracted from ELF file
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub value: u64,
    pub size: u64,
    pub name: String,
}

/// Wrapper for kernel ELF file with symbol manipulation capabilities
pub struct KernelELF {
    path: String,
    elf: Elf<'static>,
    kernel_symbols_section_name: String,
    num_kernel_symbols_name: String,
}

impl KernelELF {
    /// Load and parse a kernel ELF file
    pub fn new(
        kernel_elf_path: &str,
        kernel_symbols_section_name: &str,
        num_kernel_symbols_name: &str,
    ) -> Result<Self> {
        let path = kernel_elf_path.to_string();
        let data = fs::read(kernel_elf_path)
            .with_context(|| format!("Failed to read ELF file: {}", kernel_elf_path))?;

        // SAFETY: We need to leak the data to get a 'static lifetime for the ELF parser.
        // This is acceptable since we only create one KernelELF instance per program run.
        let static_data: &'static [u8] = Box::leak(data.into_boxed_slice());
        let elf = Elf::parse(static_data)
            .with_context(|| format!("Failed to parse ELF file: {}", kernel_elf_path))?;

        // Verify the section exists
        let _section = elf
            .section_headers
            .iter()
            .find(|sh| {
                elf.shdr_strtab
                    .get_at(sh.sh_name)
                    .map(|name| name == kernel_symbols_section_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow!(
                    "Section \"{}\" not found in ELF file",
                    kernel_symbols_section_name
                )
            })?;

        // Verify the symbol exists
        let _symbol = elf
            .syms
            .iter()
            .find(|sym| {
                elf.strtab
                    .get_at(sym.st_name)
                    .map(|name| name == num_kernel_symbols_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow!(
                    "Symbol \"{}\" not found in ELF file",
                    num_kernel_symbols_name
                )
            })?;

        Ok(KernelELF {
            path,
            elf,
            kernel_symbols_section_name: kernel_symbols_section_name.to_string(),
            num_kernel_symbols_name: num_kernel_symbols_name.to_string(),
        })
    }

    /// Get path to the ELF file
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get all non-zero-sized symbols, sorted by address
    pub fn symbols(&self) -> Result<Vec<SymbolInfo>> {
        let mut symbols: Vec<SymbolInfo> = self
            .elf
            .syms
            .iter()
            .filter(|sym| sym.st_size > 0)
            .filter_map(|sym| {
                let name = self.elf.strtab.get_at(sym.st_name)?.to_string();
                Some(SymbolInfo {
                    value: sym.st_value,
                    size: sym.st_size,
                    name,
                })
            })
            .collect();

        symbols.sort_by_key(|sym| sym.value);
        Ok(symbols)
    }

    /// Get the number of symbols
    pub fn num_symbols(&self) -> Result<usize> {
        Ok(self.symbols()?.len())
    }

    /// Get the virtual address of the kernel symbols section
    pub fn kernel_symbols_section_virt_addr(&self) -> Result<u64> {
        let section = self
            .elf
            .section_headers
            .iter()
            .find(|sh| {
                self.elf
                    .shdr_strtab
                    .get_at(sh.sh_name)
                    .map(|name| name == self.kernel_symbols_section_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow!(
                    "Section \"{}\" not found",
                    self.kernel_symbols_section_name
                )
            })?;

        Ok(section.sh_addr)
    }

    /// Get the size of the kernel symbols section
    pub fn kernel_symbols_section_size(&self) -> Result<u64> {
        let section = self
            .elf
            .section_headers
            .iter()
            .find(|sh| {
                self.elf
                    .shdr_strtab
                    .get_at(sh.sh_name)
                    .map(|name| name == self.kernel_symbols_section_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow!(
                    "Section \"{}\" not found",
                    self.kernel_symbols_section_name
                )
            })?;

        Ok(section.sh_size)
    }

    /// Get the file offset of the kernel symbols section
    pub fn kernel_symbols_section_offset_in_file(&self) -> Result<u64> {
        let virt_addr = self.kernel_symbols_section_virt_addr()?;
        self.virt_addr_to_file_offset(virt_addr)
    }

    /// Get the virtual address of the NUM_KERNEL_SYMBOLS symbol
    fn num_kernel_symbols_virt_addr(&self) -> Result<u64> {
        let symbol = self
            .elf
            .syms
            .iter()
            .find(|sym| {
                self.elf
                    .strtab
                    .get_at(sym.st_name)
                    .map(|name| name == self.num_kernel_symbols_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow!("Symbol \"{}\" not found", self.num_kernel_symbols_name)
            })?;

        Ok(symbol.st_value)
    }

    /// Get the file offset of the NUM_KERNEL_SYMBOLS symbol
    pub fn num_kernel_symbols_offset_in_file(&self) -> Result<u64> {
        let virt_addr = self.num_kernel_symbols_virt_addr()?;
        self.virt_addr_to_file_offset(virt_addr)
    }

    /// Convert a virtual address to a file offset
    fn virt_addr_to_file_offset(&self, virt_addr: u64) -> Result<u64> {
        // Find the segment containing this virtual address
        for segment in &self.elf.program_headers {
            let vaddr = segment.p_vaddr;
            let memsz = segment.p_memsz;

            if virt_addr >= vaddr && virt_addr < vaddr + memsz {
                let offset_in_segment = virt_addr - vaddr;
                let segment_file_offset = segment.p_offset;
                return Ok(segment_file_offset + offset_in_segment);
            }
        }

        Err(anyhow!(
            "Virtual address 0x{:x} not found in any segment",
            virt_addr
        ))
    }
}
