// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2020-2023 Andre Richter <andre.o.richter@gmail.com>

//! Memory Management Unit types.

use crate::{
    bsp, common,
    memory::{Address, AddressType},
};
use core::convert::From;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// A wrapper type around [Address] that ensures page alignment.
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct PageAddress<ATYPE: AddressType> {
    inner: Address<ATYPE>,
}

/// A type that describes a region of memory in quantities of pages.
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct MemoryRegion<ATYPE: AddressType> {
    start: PageAddress<ATYPE>,
    end_exclusive: PageAddress<ATYPE>,
}

/// Architecture agnostic memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

/// Architecture agnostic access permissions.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

/// Collection of memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

//------------------------------------------------------------------------------
// PageAddress
//------------------------------------------------------------------------------
impl<ATYPE: AddressType> PageAddress<ATYPE> {
    /// Unwraps the value.
    pub fn into_inner(self) -> Address<ATYPE> {
        self.inner
    }

    /// Calculates the offset from the page address.
    ///
    /// `count` is in units of [PageAddress]. For example, a count of 2 means `result = self + 2 *
    /// page_size`.
    pub fn checked_offset(self, count: isize) -> Option<Self> {
        if count == 0 {
            return Some(self);
        }

        let delta = count
            .unsigned_abs()
            .checked_mul(bsp::memory::mmu::KernelGranule::SIZE)?;
        let result = if count.is_positive() {
            self.inner.as_usize().checked_add(delta)?
        } else {
            self.inner.as_usize().checked_sub(delta)?
        };

        Some(Self {
            inner: Address::new(result),
        })
    }
}

impl<ATYPE: AddressType> From<usize> for PageAddress<ATYPE> {
    fn from(addr: usize) -> Self {
        assert!(
            common::is_aligned(addr, bsp::memory::mmu::KernelGranule::SIZE),
            "Input usize not page aligned"
        );

        Self {
            inner: Address::new(addr),
        }
    }
}

impl<ATYPE: AddressType> From<Address<ATYPE>> for PageAddress<ATYPE> {
    fn from(addr: Address<ATYPE>) -> Self {
        assert!(addr.is_page_aligned(), "Input Address not page aligned");

        Self { inner: addr }
    }
}

impl<ATYPE: AddressType> Step for PageAddress<ATYPE> {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start > end {
            return None;
        }

        // Since start <= end, do unchecked arithmetic.
        Some(
            (end.inner.as_usize() - start.inner.as_usize())
                >> bsp::memory::mmu::KernelGranule::SHIFT,
        )
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.checked_offset(count as isize)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.checked_offset(-(count as isize))
    }
}

//------------------------------------------------------------------------------
// MemoryRegion
//------------------------------------------------------------------------------
impl<ATYPE: AddressType> MemoryRegion<ATYPE> {
    /// Create an instance.
    pub fn new(start: PageAddress<ATYPE>, end_exclusive: PageAddress<ATYPE>) -> Self {
        assert!(start <= end_exclusive);

        Self {
            start,
            end_exclusive,
        }
    }

    /// Returns the start page address.
    pub fn start_page_addr(&self) -> PageAddress<ATYPE> {
        self.start
    }

    /// Returns the start address.
    pub fn start_addr(&self) -> Address<ATYPE> {
        self.start.into_inner()
    }

    /// Returns the exclusive end page address.
    pub fn end_exclusive_page_addr(&self) -> PageAddress<ATYPE> {
        self.end_exclusive
    }

    /// Returns the exclusive end page address.
    pub fn end_inclusive_page_addr(&self) -> PageAddress<ATYPE> {
        self.end_exclusive.checked_offset(-1).unwrap()
    }

    /// Checks if self contains an address.
    pub fn contains(&self, addr: Address<ATYPE>) -> bool {
        let page_addr = PageAddress::from(addr.align_down_page());
        // replacement for unstable `self.as_range().contains(&page_addr)`
        page_addr >= self.start && page_addr < self.end_exclusive
    }

    /// Checks if there is an overlap with another memory region.
    pub fn overlaps(&self, other_region: &Self) -> bool {
        // let self_range = self.as_range();

        self.contains(other_region.start_page_addr().inner)
            || self.contains(other_region.end_inclusive_page_addr().inner)
    }

    /// Returns the number of pages contained in this region.
    pub fn num_pages(&self) -> usize {
        PageAddress::steps_between(&self.start, &self.end_exclusive).unwrap()
    }

    /// Returns the size in bytes of this region.
    pub fn size(&self) -> usize {
        // Invariant: start <= end_exclusive, so do unchecked arithmetic.
        let end_exclusive = self.end_exclusive.into_inner().as_usize();
        let start = self.start.into_inner().as_usize();

        end_exclusive - start
    }
}

impl<ATYPE: AddressType> IntoIterator for MemoryRegion<ATYPE> {
    type Item = PageAddress<ATYPE>;
    type IntoIter = PageAddressIterator<ATYPE>;

    fn into_iter(self) -> Self::IntoIter {
        PageAddressIterator {
            start: self.start,
            end: self.end_exclusive,
        }
    }
}

/// Iterator over a range of PageAddresses.
///
/// This type is a workaround until the standard library's `Step` trait is stable. Then we can use
/// the `Range` type instead.
pub struct PageAddressIterator<T>
where
    T: AddressType,
{
    start: PageAddress<T>,
    end: PageAddress<T>,
}

impl<T> Iterator for PageAddressIterator<T>
where
    T: AddressType,
{
    type Item = PageAddress<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let old = self.start;
            self.start = Step::forward(old, 1);
            Some(old)
        } else {
            None
        }
    }
}

/// Copy of unstable `core::iter::Step` trait
///
/// From Rust 1.83. There were breaking changes in 1.84.
#[allow(missing_docs)] // see standard library
pub trait Step: Clone + PartialOrd + Sized {
    fn steps_between(start: &Self, end: &Self) -> Option<usize>;
    fn forward_checked(start: Self, count: usize) -> Option<Self>;
    fn forward(start: Self, count: usize) -> Self {
        Step::forward_checked(start, count).expect("overflow in `Step::forward`")
    }
    fn backward_checked(start: Self, count: usize) -> Option<Self>;
    fn backward(start: Self, count: usize) -> Self {
        Step::backward_checked(start, count).expect("overflow in `Step::backward`")
    }
}
