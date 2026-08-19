// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2021-2023 Andre Richter <andre.o.richter@gmail.com>

//! Translation table.

#[cfg(target_arch = "aarch64")]
#[path = "../../_arch/aarch64/memory/mmu/translation_table.rs"]
mod arch_translation_table;

use super::{AttributeFields, MemoryRegion};
use crate::memory::{Address, Physical, Virtual};

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Translation table interfaces.
pub mod interface {
    use crate::memory::mmu::PageAddress;

    use super::*;

    /// Translation table operations.
    pub trait TranslationTable {
        /// Anything that needs to run before any of the other provided functions can be used.
        ///
        /// # Safety
        ///
        /// - Implementor must ensure that this function can run only once or is harmless if invoked
        ///   multiple times.
        #[allow(dead_code)]
        fn init(&mut self) -> Result<(), &'static str>;

        /// Map the given virtual memory region to the given physical memory region.
        ///
        /// # Safety
        ///
        /// - Using wrong attributes can cause multiple issues of different nature in the system.
        /// - It is not required that the architectural implementation prevents aliasing. That is,
        ///   mapping to the same physical memory using multiple virtual addresses, which would
        ///   break Rust's ownership assumptions. This should be protected against in the kernel's
        ///   generic MMU code.
        unsafe fn map_at(
            &mut self,
            virt_region: &MemoryRegion<Virtual>,
            phys_region: &MemoryRegion<Physical>,
            attr: &AttributeFields,
        ) -> Result<(), &'static str>;

        /// Try to translate a virtual page address to a physical page address.
        ///
        /// Will only succeed if there exists a valid mapping for the input page.
        fn try_virt_page_addr_to_phys_page_addr(
            &self,
            virt_page_addr: PageAddress<Virtual>,
        ) -> Result<PageAddress<Physical>, &'static str>;

        /// Try to get the attributes of a page.
        ///
        /// Will only succeed if there exists a valid mapping for the input page.
        fn try_page_attributes(
            &self,
            virt_page_addr: PageAddress<Virtual>,
        ) -> Result<AttributeFields, &'static str>;

        /// Try to translate a virtual address to a physical address.
        ///
        /// Will only succeed if there exists a valid mapping for the input address.
        fn try_virt_addr_to_phys_addr(
            &self,
            virt_addr: Address<Virtual>,
        ) -> Result<Address<Physical>, &'static str>;
    }
}
