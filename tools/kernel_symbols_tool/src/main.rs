#!/usr/bin/env rust
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2022-2023 Andre Richter <andre.o.richter@gmail.com>

mod commands;
mod kernel_elf;

use anyhow::{anyhow, Context, Result};
use kernel_elf::KernelELF;
use std::env;

const KERNEL_SYMBOLS_SECTION: &str = ".kernel_symbols";
const NUM_KERNEL_SYMBOLS: &str = "NUM_KERNEL_SYMBOLS";

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  kernel-elf-symbol --gen_symbols <kernel_elf> <output_file>");
    eprintln!("  kernel-elf-symbol --get_symbols_section_virt_addr <kernel_elf>");
    eprintln!("  kernel-elf-symbol --patch_data <kernel_elf> <symbols_blob>");
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        print_usage();
        return Err(anyhow!("Insufficient arguments"));
    }

    let cmd = &args[1];
    let kernel_elf_path = &args[2];

    match cmd.as_str() {
        "--gen_symbols" => {
            if args.len() < 4 {
                print_usage();
                return Err(anyhow!("Missing output file argument for --gen_symbols"));
            }

            let output_file = &args[3];

            println!("\x1b[1m\x1b[32m{:>12}\x1b[0m Symbols source file", "Generating");

            let kernel_elf = KernelELF::new(kernel_elf_path, KERNEL_SYMBOLS_SECTION, NUM_KERNEL_SYMBOLS)
                .context("Failed to load kernel ELF")?;

            commands::generate_symbols(&kernel_elf, output_file)?;
        }

        "--get_symbols_section_virt_addr" => {
            let kernel_elf = KernelELF::new(kernel_elf_path, KERNEL_SYMBOLS_SECTION, NUM_KERNEL_SYMBOLS)
                .context("Failed to load kernel ELF")?;

            let addr = commands::get_symbols_section_virt_addr(&kernel_elf)?;
            println!("0x{:x}", addr);
        }

        "--patch_data" => {
            if args.len() < 4 {
                print_usage();
                return Err(anyhow!("Missing symbols blob argument for --patch_data"));
            }

            let symbols_blob_path = &args[3];

            let kernel_elf = KernelELF::new(kernel_elf_path, KERNEL_SYMBOLS_SECTION, NUM_KERNEL_SYMBOLS)
                .context("Failed to load kernel ELF")?;

            let num_symbols = kernel_elf.num_symbols()?;

            println!(
                "\x1b[1m\x1b[32m{:>12}\x1b[0m Symbols blob and number of symbols ({}) into ELF",
                "Patching", num_symbols
            );

            commands::patch_symbol_data(&kernel_elf, symbols_blob_path)?;
            commands::patch_num_symbols(&kernel_elf)?;
        }

        _ => {
            print_usage();
            return Err(anyhow!("Unknown command: {}", cmd));
        }
    }

    Ok(())
}
