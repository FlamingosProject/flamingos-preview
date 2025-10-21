use crate::memory::{
    AccessPermissions, AttributeFields, MemoryAttributes, MemoryRegion, 
    GRANULE_64KIB_SHIFT, GRANULE_512MIB_MASK, GRANULE_512MIB_SHIFT
};
use crate::Result;
use anyhow::anyhow;

#[derive(Debug, Clone, Copy)]
pub struct Stage1TableDescriptor {
    value: u64,
}

impl Stage1TableDescriptor {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn set_next_level_table_addr(&mut self, addr: u64) {
        let addr_shifted = addr >> GRANULE_64KIB_SHIFT;
        self.value &= !(0xFFFFFFFF << 16); // Clear bits 47:16
        self.value |= (addr_shifted & 0xFFFFFFFF) << 16;
    }

    pub fn set_type(&mut self, table_type: TableType) {
        self.value &= !(1 << 1); // Clear bit 1
        self.value |= (table_type as u64) << 1;
    }

    pub fn set_valid(&mut self, valid: bool) {
        self.value &= !1; // Clear bit 0
        self.value |= valid as u64;
    }

    pub fn to_u64(self) -> u64 {
        self.value
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TableType {
    Block = 0,
    Table = 1,
}

#[derive(Debug, Clone, Copy)]
pub struct Stage1PageDescriptor {
    value: u64,
}

impl Stage1PageDescriptor {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn set_uxn(&mut self, uxn: bool) {
        self.value &= !(1u64 << 54); // Clear bit 54
        self.value |= (uxn as u64) << 54;
    }

    pub fn set_pxn(&mut self, pxn: bool) {
        self.value &= !(1u64 << 53); // Clear bit 53
        self.value |= (pxn as u64) << 53;
    }

    pub fn set_output_addr(&mut self, addr: u64) {
        let addr_shifted = addr >> GRANULE_64KIB_SHIFT;
        self.value &= !(0xFFFFFFFF << 16); // Clear bits 47:16
        self.value |= (addr_shifted & 0xFFFFFFFF) << 16;
    }

    pub fn set_af(&mut self, af: bool) {
        self.value &= !(1 << 10); // Clear bit 10
        self.value |= (af as u64) << 10;
    }

    pub fn set_sh(&mut self, sh: SharabilityField) {
        self.value &= !(0b11 << 8); // Clear bits 9:8
        self.value |= (sh as u64) << 8;
    }

    pub fn set_ap(&mut self, ap: AccessPermissionsField) {
        self.value &= !(0b11 << 6); // Clear bits 7:6
        self.value |= (ap as u64) << 6;
    }

    pub fn set_attr_indx(&mut self, attr_indx: u8) {
        self.value &= !(0b111 << 2); // Clear bits 4:2
        self.value |= (attr_indx as u64 & 0b111) << 2;
    }

    pub fn set_type(&mut self, page_type: PageType) {
        self.value &= !(1 << 1); // Clear bit 1
        self.value |= (page_type as u64) << 1;
    }

    pub fn set_valid(&mut self, valid: bool) {
        self.value &= !1; // Clear bit 0
        self.value |= valid as u64;
    }

    pub fn to_u64(self) -> u64 {
        self.value
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SharabilityField {
    InnerShareable = 0b11,
}

#[derive(Debug, Clone, Copy)]  
pub enum AccessPermissionsField {
    RwEl1 = 0b00,
    RoEl1 = 0b10,
}

#[derive(Debug, Clone, Copy)]
pub enum PageType {
    ReservedInvalid = 0,
    Page = 1,
}

const MAIR_NORMAL: u8 = 1;

pub struct TranslationTable {
    lvl3_tables: Vec<Vec<Stage1PageDescriptor>>,
    lvl2_tables: Vec<Stage1TableDescriptor>,
    lvl2_phys_start_addr: u64,
    kernel_virt_start_addr: u64,
}

impl TranslationTable {
    pub fn new(
        kernel_virt_addr_space_size: usize,
        kernel_virt_start_addr: u64,
        phys_addr_of_kernel_tables: u64,
    ) -> Result<Self> {
        // Sanity checks
        if (kernel_virt_addr_space_size % (512 * 1024 * 1024)) != 0 {
            return Err(anyhow!("Kernel virtual address space size must be aligned to 512 MiB"));
        }

        let num_lvl2_tables = kernel_virt_addr_space_size >> GRANULE_512MIB_SHIFT;

        // Create level 3 tables (8192 entries each)
        let mut lvl3_tables = Vec::new();
        let mut current_addr = phys_addr_of_kernel_tables;

        for _ in 0..num_lvl2_tables {
            let table = vec![Stage1PageDescriptor::new(); 8192];
            lvl3_tables.push(table);
            current_addr += 8192 * 8; // Each descriptor is 8 bytes
        }

        let lvl2_phys_start_addr = current_addr;

        // Create level 2 tables
        let mut lvl2_tables = vec![Stage1TableDescriptor::new(); num_lvl2_tables];

        // Populate level 2 entries
        let mut lvl3_phys_addr = phys_addr_of_kernel_tables;
        for desc in &mut lvl2_tables {
            desc.set_next_level_table_addr(lvl3_phys_addr);
            desc.set_type(TableType::Table);
            desc.set_valid(true);
            lvl3_phys_addr += 8192 * 8;
        }

        Ok(Self {
            lvl3_tables,
            lvl2_tables,
            lvl2_phys_start_addr,
            kernel_virt_start_addr,
        })
    }

    pub fn map_at(
        &mut self,
        virt_region: &MemoryRegion,
        phys_region: &MemoryRegion,
        attributes: &AttributeFields,
    ) -> Result<()> {
        if virt_region.is_empty() {
            return Ok(());
        }

        if virt_region.len() != phys_region.len() {
            return Err(anyhow!("Virtual and physical regions must have the same size"));
        }

        for (virt_page, phys_page) in virt_region.zip_with(phys_region) {
            let (lvl2_index, lvl3_index) = self.lvl2_lvl3_index_from(virt_page)?;
            Self::set_lvl3_entry(&mut self.lvl3_tables[lvl2_index][lvl3_index], phys_page, attributes)?;
        }

        Ok(())
    }

    pub fn to_binary(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Add all level 3 table entries
        for table in &self.lvl3_tables {
            for desc in table {
                data.extend_from_slice(&desc.to_u64().to_le_bytes());
            }
        }

        // Add level 2 table entries
        for desc in &self.lvl2_tables {
            data.extend_from_slice(&desc.to_u64().to_le_bytes());
        }

        data
    }

    pub fn phys_tables_base_addr_binary(&self) -> Vec<u8> {
        self.lvl2_phys_start_addr.to_le_bytes().to_vec()
    }

    pub fn phys_tables_base_addr(&self) -> u64 {
        self.lvl2_phys_start_addr
    }

    fn lvl2_lvl3_index_from(&self, addr: u64) -> Result<(usize, usize)> {
        // Subtract kernel_virt_start_addr to get offset from kernel virtual start
        let addr = addr - self.kernel_virt_start_addr;

        let lvl2_index = (addr >> GRANULE_512MIB_SHIFT) as usize;
        let lvl3_index = ((addr & GRANULE_512MIB_MASK as u64) >> GRANULE_64KIB_SHIFT) as usize;

        if lvl2_index >= self.lvl2_tables.len() {
            return Err(anyhow!("Level 2 index {} out of bounds", lvl2_index));
        }

        Ok((lvl2_index, lvl3_index))
    }

    fn set_attributes(desc: &mut Stage1PageDescriptor, attributes: &AttributeFields) -> Result<()> {
        match attributes.mem_attributes {
            MemoryAttributes::CacheableDRAM => {
                desc.set_sh(SharabilityField::InnerShareable);
                desc.set_attr_indx(MAIR_NORMAL);
            }
        }

        let ap = match attributes.acc_perms {
            AccessPermissions::ReadOnly => AccessPermissionsField::RoEl1,
            AccessPermissions::ReadWrite => AccessPermissionsField::RwEl1,
        };
        desc.set_ap(ap);

        desc.set_pxn(attributes.execute_never);
        desc.set_uxn(true); // Always set UXN to true

        Ok(())
    }

    fn set_lvl3_entry(
        desc: &mut Stage1PageDescriptor,
        output_addr: u64,
        attributes: &AttributeFields,
    ) -> Result<()> {
        desc.set_output_addr(output_addr);
        desc.set_af(true);
        desc.set_type(PageType::Page);
        desc.set_valid(true);

        Self::set_attributes(desc, attributes)?;

        Ok(())
    }
}