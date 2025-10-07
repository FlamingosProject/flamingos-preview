use crate::elf::KernelELF;
use crate::memory::GRANULE_64KIB_SIZE;
use crate::Result;
use anyhow::anyhow;
use regex::Regex;
use std::fs;

#[derive(Debug, Clone, Copy)]
pub enum BspType {
    Rpi3,
    Rpi4,
}

impl BspType {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "rpi3" => Ok(BspType::Rpi3),
            "rpi4" => Ok(BspType::Rpi4),
            _ => Err(anyhow!("Unknown BSP type: {}", s)),
        }
    }
}

pub struct RaspberryPi {
    bsp_type: BspType,
    kernel_virt_addr_space_size: usize,
    virt_addr_of_kernel_tables: u64,
    virt_addr_of_phys_kernel_tables_base_addr: u64,
}

impl RaspberryPi {
    pub fn new(bsp_type: BspType, kernel_elf: &KernelELF) -> Result<Self> {
        let kernel_virt_addr_space_size = kernel_elf.symbol_value("__kernel_virt_addr_space_size")? as usize;
        let virt_addr_of_kernel_tables = kernel_elf.symbol_value("KERNEL_TABLES")?;
        let virt_addr_of_phys_kernel_tables_base_addr = kernel_elf.symbol_value("PHYS_KERNEL_TABLES_BASE_ADDR")?;

        Ok(Self {
            bsp_type,
            kernel_virt_addr_space_size,
            virt_addr_of_kernel_tables,
            virt_addr_of_phys_kernel_tables_base_addr,
        })
    }

    pub fn kernel_granule_size(&self) -> usize {
        GRANULE_64KIB_SIZE
    }

    pub fn kernel_virt_addr_space_size(&self) -> usize {
        self.kernel_virt_addr_space_size
    }

    pub fn phys_addr_of_kernel_tables(&self, kernel_elf: &KernelELF) -> Result<u64> {
        kernel_elf.virt_to_phys(self.virt_addr_of_kernel_tables)
    }

    pub fn kernel_tables_offset_in_file(&self, kernel_elf: &KernelELF) -> Result<u64> {
        kernel_elf.virt_addr_to_file_offset(self.virt_addr_of_kernel_tables)
    }

    pub fn phys_kernel_tables_base_addr_offset_in_file(&self, kernel_elf: &KernelELF) -> Result<u64> {
        kernel_elf.virt_addr_to_file_offset(self.virt_addr_of_phys_kernel_tables_base_addr)
    }

    pub fn phys_addr_space_end_page(&self) -> Result<u64> {
        let memory_rs_content = fs::read_to_string("kernel/src/bsp/raspberrypi/memory.rs")
            .map_err(|e| anyhow!("Failed to read memory.rs: {}", e))?;

        let re = Regex::new(r"pub const END.*?=.*?0x([0-9a-fA-F]+)")
            .map_err(|e| anyhow!("Failed to compile regex: {}", e))?;

        let end_values: Vec<&str> = re
            .captures_iter(&memory_rs_content)
            .map(|cap| cap.get(1).unwrap().as_str())
            .collect();

        if end_values.is_empty() {
            return Err(anyhow!("No END constants found in memory.rs"));
        }

        let value_str = match self.bsp_type {
            BspType::Rpi3 => {
                if end_values.is_empty() {
                    return Err(anyhow!("No END constants found for RPI3"));
                }
                end_values[0]
            }
            BspType::Rpi4 => {
                if end_values.len() < 2 {
                    return Err(anyhow!("Not enough END constants found for RPI4"));
                }
                end_values[1]
            }
        };

        u64::from_str_radix(value_str, 16)
            .map_err(|e| anyhow!("Failed to parse hex value '{}': {}", value_str, e))
    }
}