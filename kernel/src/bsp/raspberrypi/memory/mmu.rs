// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! BSP Memory Management Unit.

#[cfg(not(feature = "chainloader"))]
use crate::memory::{
    Address,
    mmu::{AccessPermissions, MemAttributes, TranslationTable},
};
use crate::{
    memory::{
        Physical, Virtual,
        mmu::{
            self as generic_mmu, AddressSpace, AssociatedTranslationTable, AttributeFields,
            MemoryRegion, PageAddress, TranslationGranule,
        },
    },
    synchronization::InitStateLock,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

type KernelTranslationTable =
    <KernelVirtAddrSpace as AssociatedTranslationTable>::TableStartFromTop;

/// A mapping according to the kernel's linker script.
#[cfg(not(feature = "chainloader"))]
#[repr(C)]
#[derive(Clone, Copy)]
struct BootRegion {
    virt_start: usize,
    virt_end_exclusive: usize,
    phys_start: usize,
}

/// Initial mappings emitted by the linker into `.text` for the physical bootstrap.
#[cfg(not(feature = "chainloader"))]
#[repr(C)]
pub(crate) struct BootMemoryLayout {
    code: BootRegion,
    data: BootRegion,
    heap: BootRegion,
    stack: BootRegion,
    retained_elf: BootRegion,
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// The translation granule chosen by this BSP. This will be used everywhere else in the kernel to
/// derive respective data structures and their sizes. For example, the `crate::memory::mmu::Page`.
pub type KernelGranule = TranslationGranule<{ 64 * 1024 }>;

/// The kernel's virtual address space defined by this BSP.
pub type KernelVirtAddrSpace = AddressSpace<{ kernel_virt_addr_space_size() }>;

//--------------------------------------------------------------------------------------------------
// Global instances
//--------------------------------------------------------------------------------------------------

/// The zeroed kernel translation tables constructed during physical boot.
///
/// # Safety
/// This symbol is unique in the kernel ELF. All-zero storage is valid for the
/// inactive table, and the linker preserves the table's alignment in BSS.
///
/// # Need
/// The linker resolves the storage's physical address for boot initialization.
#[unsafe(link_section = ".bss.kernel_tables")]
#[unsafe(no_mangle)]
static KERNEL_TABLES: InitStateLock<KernelTranslationTable> =
    InitStateLock::new(KernelTranslationTable::new_uninitialized());

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

/// This is a hack for retrieving the value for the kernel's virtual address space size as a
/// constant from a common place, since it is needed as a compile-time/link-time constant in both,
/// the linker script and the Rust sources.
#[allow(clippy::needless_late_init)]
const fn kernel_virt_addr_space_size() -> usize {
    let __kernel_virt_addr_space_size;

    include!(concat!(
        env!("LD_SCRIPT_PATH"),
        "/kernel_virt_addr_space_size.ld",
    ));

    __kernel_virt_addr_space_size
}

/// Helper function for calculating the number of pages the given parameter spans.
const fn size_to_num_pages(size: usize) -> usize {
    assert!(size > 0);
    assert!(size.is_multiple_of(KernelGranule::SIZE));

    size >> KernelGranule::SHIFT
}

/// The data pages of the kernel binary.
fn virt_data_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::data_size());

    let start_page_addr = super::virt_data_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

// These conversions read the established bootstrap mappings.
fn kernel_virt_to_phys_region(virt_region: MemoryRegion<Virtual>) -> MemoryRegion<Physical> {
    let phys_start_page_addr =
        generic_mmu::try_kernel_virt_page_addr_to_phys_page_addr(virt_region.start_page_addr())
            .unwrap();

    let phys_end_exclusive_page_addr = phys_start_page_addr
        .checked_offset(virt_region.num_pages() as isize)
        .unwrap();

    MemoryRegion::new(phys_start_page_addr, phys_end_exclusive_page_addr)
}

fn kernel_page_attributes(virt_page_addr: PageAddress<Virtual>) -> AttributeFields {
    generic_mmu::try_kernel_page_attributes(virt_page_addr).unwrap()
}

#[cfg(not(feature = "chainloader"))]
impl BootRegion {
    /// Install a linker-defined range in the inactive kernel tables.
    ///
    /// # Safety
    /// The caller must ensure the range describes its loaded physical backing,
    /// is disjoint from other mappings, and uses permissions appropriate to it.
    ///
    /// # Need
    /// Installing page descriptors configures the CPU's memory-access rules.
    unsafe fn map(
        &self,
        tables: &mut KernelTranslationTable,
        permissions: AccessPermissions,
        executable: bool,
    ) -> Result<(), &'static str> {
        let size = self.virt_end_exclusive - self.virt_start;
        let virtual_region = MemoryRegion::new(
            PageAddress::from(self.virt_start),
            PageAddress::from(self.virt_end_exclusive),
        );
        let physical_region = MemoryRegion::new(
            PageAddress::from(self.phys_start),
            PageAddress::from(self.phys_start + size),
        );
        let attributes = AttributeFields {
            mem_attributes: MemAttributes::CacheableDRAM,
            acc_perms: permissions,
            execute_never: !executable,
        };
        // Safety: The caller validates the backing, disjointness and permissions.
        // Need: These descriptors will be installed as the kernel translation tables.
        unsafe { tables.map_at(&virtual_region, &physical_region, &attributes) }
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// The code pages of the kernel binary.
pub fn virt_code_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::code_size());

    let start_page_addr = super::virt_code_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// The heap pages.
pub fn virt_heap_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::heap_size());

    let start_page_addr = super::virt_heap_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// The boot core stack pages.
pub fn virt_boot_core_stack_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::boot_core_stack_size());

    let start_page_addr = super::virt_boot_core_stack_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// Return a reference to the kernel's translation tables.
pub fn kernel_translation_tables() -> &'static InitStateLock<KernelTranslationTable> {
    &KERNEL_TABLES
}

/// Construct the initial mappings while running at physical addresses.
///
/// # Safety
/// The boot core must have exclusive access to the zeroed tables. `tables`
/// points to the physical `KERNEL_TABLES`, `layout` describes the loaded kernel,
/// and any retained ELF bytes occupy the reserved physical staging area.
///
/// # Need
/// The physical bootstrap must populate hardware translation tables before
/// the kernel can use the mapped address of its table storage.
#[cfg(not(feature = "chainloader"))]
pub(crate) unsafe fn initialize_boot_tables(
    tables: *const (),
    layout: &BootMemoryLayout,
    elf_size: usize,
) -> Result<Address<Physical>, &'static str> {
    let elf = layout.retained_elf;
    if elf_size > elf.virt_end_exclusive - elf.virt_start {
        return Err("Boot ELF exceeds retained mapping reservation");
    }

    // Safety: Only the boot core accesses these inactive tables. The linker
    // supplies disjoint ranges backed by the loaded kernel and its retained ELF.
    // Need: Only the physical pointer is usable before address translation.
    unsafe {
        let tables = &*tables.cast::<InitStateLock<KernelTranslationTable>>();
        tables.with_boot_mut(|tables| {
            let root = tables.init_for_boot()?;
            layout.code.map(tables, AccessPermissions::ReadOnly, true)?;
            for region in [&layout.data, &layout.heap, &layout.stack] {
                region.map(tables, AccessPermissions::ReadWrite, false)?;
            }
            if elf_size > 0 {
                let retained = BootRegion {
                    virt_end_exclusive: elf.virt_start
                        + crate::common::align_up(elf_size, KernelGranule::SIZE),
                    ..elf
                };
                retained.map(tables, AccessPermissions::ReadOnly, false)?;
            }
            Ok(root)
        })
    }
}

/// The MMIO remap pages.
pub fn virt_mmio_remap_region() -> MemoryRegion<Virtual> {
    let num_pages = size_to_num_pages(super::mmio_remap_size());

    let start_page_addr = super::virt_mmio_remap_start();
    let end_exclusive_page_addr = start_page_addr.checked_offset(num_pages as isize).unwrap();

    MemoryRegion::new(start_page_addr, end_exclusive_page_addr)
}

/// Record the mappings installed during the physical kernel bootstrap.
pub fn kernel_add_mapping_records_for_boot() {
    let virt_code_region = virt_code_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel code and RO data",
        &virt_code_region,
        &kernel_virt_to_phys_region(virt_code_region),
        &kernel_page_attributes(virt_code_region.start_page_addr()),
    );

    let virt_data_region = virt_data_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel data and bss",
        &virt_data_region,
        &kernel_virt_to_phys_region(virt_data_region),
        &kernel_page_attributes(virt_data_region.start_page_addr()),
    );

    let virt_heap_region = virt_heap_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel heap",
        &virt_heap_region,
        &kernel_virt_to_phys_region(virt_heap_region),
        &kernel_page_attributes(virt_heap_region.start_page_addr()),
    );

    let virt_boot_core_stack_region = virt_boot_core_stack_region();
    generic_mmu::kernel_add_mapping_record(
        "Kernel boot-core stack",
        &virt_boot_core_stack_region,
        &kernel_virt_to_phys_region(virt_boot_core_stack_region),
        &kernel_page_attributes(virt_boot_core_stack_region.start_page_addr()),
    );

    #[cfg(not(feature = "chainloader"))]
    if let Some(elf) = crate::cpu::boot::arch_boot::retained_kernel_elf() {
        let start = Address::new(elf.as_ptr() as usize);
        let virt_region = MemoryRegion::new(
            PageAddress::from(start),
            PageAddress::from((start + elf.len()).align_up_page()),
        );
        generic_mmu::kernel_add_mapping_record(
            "Retained kernel ELF",
            &virt_region,
            &kernel_virt_to_phys_region(virt_region),
            &kernel_page_attributes(virt_region.start_page_addr()),
        );
    }
}
