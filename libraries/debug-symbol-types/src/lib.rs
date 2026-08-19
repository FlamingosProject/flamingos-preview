//! Debug symbol types for kernel debugging

#![no_std]

/// Represents a symbol in the kernel symbol table
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Symbol {
    addr: u64,
    size: u64,
    name: &'static str,
}

impl Symbol {
    /// Create a new symbol
    pub const fn new(addr: u64, size: u64, name: &'static str) -> Self {
        Symbol { addr, size, name }
    }

    /// Get the address of this symbol
    pub const fn addr(&self) -> u64 {
        self.addr
    }

    /// Get the size of this symbol
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Get the name of this symbol
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Check if the given address is contained within this symbol
    pub const fn contains(&self, addr: usize) -> bool {
        let addr = addr as u64;
        addr >= self.addr && addr < (self.addr + self.size)
    }
}
