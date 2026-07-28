use clap::Parser;
use colored::Colorize;
use std::time::Instant;
use translation_table_tool::{
    Result,
    arch::TranslationTable,
    bsp::{BspType, RaspberryPi},
    elf::KernelELF,
    translation::{kernel_map_binary, kernel_patch_base_addr, kernel_patch_tables},
};

#[derive(Parser)]
#[command(name = "translation_table_tool")]
#[command(about = "Translation table generator for OS kernel")]
struct Args {
    /// BSP type (rpi3 or rpi4)
    bsp_type: String,

    /// Path to kernel ELF file
    kernel_elf_path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let start = Instant::now();

    // Parse BSP type
    let bsp_type = BspType::from_str(&args.bsp_type)?;

    // Load kernel ELF
    let kernel_elf = KernelELF::new(&args.kernel_elf_path)?;

    // Verify architecture
    if kernel_elf.machine() != "AArch64" {
        return Err(translation_table_tool::anyhow!(
            "Unsupported architecture: {}",
            kernel_elf.machine()
        ));
    }

    // Initialize BSP
    let bsp = RaspberryPi::new(bsp_type, &kernel_elf)?;

    // Create translation tables
    let kernel_virt_addr_space_size = bsp.kernel_virt_addr_space_size();
    let phys_addr_of_kernel_tables = bsp.phys_addr_of_kernel_tables(&kernel_elf)?;

    let mut translation_tables =
        TranslationTable::new(kernel_virt_addr_space_size, phys_addr_of_kernel_tables)?;

    // Generate mapping descriptors
    let mapping_descriptors = kernel_elf.generate_mapping_descriptors()?;

    // Map binary sections
    kernel_map_binary(&mut translation_tables, &mapping_descriptors)?;

    // Patch kernel ELF file
    kernel_patch_tables(
        &args.kernel_elf_path,
        &translation_tables,
        &bsp,
        &kernel_elf,
    )?;
    kernel_patch_base_addr(
        &args.kernel_elf_path,
        &translation_tables,
        &bsp,
        &kernel_elf,
    )?;

    let elapsed = start.elapsed();
    print!("{}", format!("{:>12}", "Finished").green().bold());
    println!(" in {:.2}s", elapsed.as_secs_f64());

    Ok(())
}
