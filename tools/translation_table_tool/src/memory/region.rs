use crate::memory::Alignment;
use crate::Result;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pages: Vec<u64>,
}

impl MemoryRegion {
    pub fn new(start_addr: u64, size: usize, granule_size: usize) -> Result<Self> {
        if !start_addr.is_aligned(granule_size as u64) {
            return Err(anyhow!("Start address not aligned to granule size"));
        }
        
        if size == 0 {
            return Err(anyhow!("Size must be positive"));
        }
        
        if size % granule_size != 0 {
            return Err(anyhow!("Size must be aligned to granule size"));
        }

        let num_pages = size / granule_size;
        let pages = (0..num_pages)
            .map(|i| start_addr + (i * granule_size) as u64)
            .collect();

        Ok(Self { pages })
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    pub fn first(&self) -> Option<u64> {
        self.pages.first().copied()
    }

    pub fn last(&self) -> Option<u64> {
        self.pages.last().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.pages.iter().copied()
    }

    pub fn zip_with<'a>(&'a self, other: &'a MemoryRegion) -> impl Iterator<Item = (u64, u64)> + 'a {
        self.pages.iter().zip(other.pages.iter()).map(|(&a, &b)| (a, b))
    }
}