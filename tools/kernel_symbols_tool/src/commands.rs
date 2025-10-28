// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

use crate::kernel_elf::KernelELF;
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

/// Generate a Rust source file containing kernel symbols
pub fn generate_symbols(kernel_elf: &KernelELF, output_file: &str) -> Result<()> {
    let symbols = kernel_elf.symbols()?;
    let num_symbols = symbols.len();

    let mut file = File::create(output_file)
        .with_context(|| format!("Failed to create output file: {}", output_file))?;

    // Write header
    writeln!(file, "use debug_symbol_types::Symbol;")?;
    writeln!(file)?;
    writeln!(file, "# [no_mangle]")?;
    writeln!(file, "# [link_section = \".rodata.symbol_desc\"]")?;
    writeln!(
        file,
        "static KERNEL_SYMBOLS: [Symbol; {}] = [",
        num_symbols
    )?;

    // Write each symbol
    for sym in symbols {
        writeln!(
            file,
            "    Symbol::new({}, {}, \"{}\"),",
            sym.value, sym.size, sym.name
        )?;
    }

    writeln!(file, "];")?;

    Ok(())
}

/// Get the virtual address of the kernel symbols section
pub fn get_symbols_section_virt_addr(kernel_elf: &KernelELF) -> Result<u64> {
    kernel_elf.kernel_symbols_section_virt_addr()
}

/// Patch symbol data blob into the ELF file
pub fn patch_symbol_data(kernel_elf: &KernelELF, symbols_blob_path: &str) -> Result<()> {
    let symbols_blob = std::fs::read(symbols_blob_path)
        .with_context(|| format!("Failed to read symbols blob: {}", symbols_blob_path))?;

    let section_size = kernel_elf.kernel_symbols_section_size()?;
    if symbols_blob.len() as u64 > section_size {
        anyhow::bail!(
            "Symbol blob size ({} bytes) exceeds section size ({} bytes)",
            symbols_blob.len(),
            section_size
        );
    }

    let offset = kernel_elf.kernel_symbols_section_offset_in_file()?;

    // Open file for reading and writing
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .open(kernel_elf.path())
        .with_context(|| format!("Failed to open ELF file for patching: {}", kernel_elf.path()))?;

    // Seek to the correct offset and write the blob
    file.seek(SeekFrom::Start(offset))
        .context("Failed to seek to kernel symbols section offset")?;
    file.write_all(&symbols_blob)
        .context("Failed to write symbols blob to ELF file")?;

    Ok(())
}

/// Patch the number of symbols as a uint64_t into the ELF file
pub fn patch_num_symbols(kernel_elf: &KernelELF) -> Result<()> {
    let num_symbols = kernel_elf.num_symbols()? as u64;
    let num_packed = num_symbols.to_le_bytes(); // Little-endian uint64_t

    let offset = kernel_elf.num_kernel_symbols_offset_in_file()?;

    // Open file for reading and writing
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .open(kernel_elf.path())
        .with_context(|| format!("Failed to open ELF file for patching: {}", kernel_elf.path()))?;

    // Seek to the correct offset and write the number
    file.seek(SeekFrom::Start(offset))
        .context("Failed to seek to NUM_KERNEL_SYMBOLS offset")?;
    file.write_all(&num_packed)
        .context("Failed to write number of symbols to ELF file")?;

    Ok(())
}
