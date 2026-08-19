use crate::Result;
use crate::arch::TranslationTable;
use crate::bsp::RaspberryPi;
use crate::elf::{KernelELF, MappingDescriptor};
use crate::memory::Alignment;
use colored::Colorize;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

pub fn kernel_map_binary(
    translation_tables: &mut TranslationTable,
    mapping_descriptors: &[MappingDescriptor],
) -> Result<()> {
    if let Some(first_desc) = mapping_descriptors.first() {
        MappingDescriptor::print_header(first_desc.max_section_name_length);
    }

    for desc in mapping_descriptors {
        print!("{}", format!("{:>12}", "Generating").green().bold());
        print!(" ");
        println!("{}", desc);

        translation_tables.map_at(&desc.virt_region, &desc.phys_region, &desc.attributes)?;
    }

    if let Some(first_desc) = mapping_descriptors.first() {
        MappingDescriptor::print_divider(first_desc.max_section_name_length);
    }

    Ok(())
}

pub fn kernel_patch_tables(
    kernel_elf_path: &str,
    translation_tables: &TranslationTable,
    bsp: &RaspberryPi,
    kernel_elf: &KernelELF,
) -> Result<()> {
    let offset = bsp.kernel_tables_offset_in_file(kernel_elf)?;

    print!("{}", format!("{:>12}", "Patching").green().bold());
    print!(" Kernel table struct at ELF file offset ");
    println!("{}", offset.to_hex_underscore(false));

    let binary_data = translation_tables.to_binary();

    let mut file = OpenOptions::new().write(true).open(kernel_elf_path)?;

    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&binary_data)?;

    Ok(())
}

pub fn kernel_patch_base_addr(
    kernel_elf_path: &str,
    translation_tables: &TranslationTable,
    bsp: &RaspberryPi,
    kernel_elf: &KernelELF,
) -> Result<()> {
    let offset = bsp.phys_kernel_tables_base_addr_offset_in_file(kernel_elf)?;
    let base_addr = translation_tables.phys_tables_base_addr();

    print!("{}", format!("{:>12}", "Patching").green().bold());
    print!(" Kernel tables physical base address start argument to value ");
    print!("{}", base_addr.to_hex_underscore(false));
    print!(" at ELF file offset ");
    println!("{}", offset.to_hex_underscore(false));

    let binary_data = translation_tables.phys_tables_base_addr_binary();

    let mut file = OpenOptions::new().write(true).open(kernel_elf_path)?;

    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&binary_data)?;

    Ok(())
}
