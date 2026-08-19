use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum MemoryAttributes {
    CacheableDRAM,
}

#[derive(Debug, Clone, Copy)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy)]
pub struct AttributeFields {
    pub mem_attributes: MemoryAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

impl AttributeFields {
    pub fn new(
        mem_attributes: MemoryAttributes,
        acc_perms: AccessPermissions,
        execute_never: bool,
    ) -> Self {
        Self {
            mem_attributes,
            acc_perms,
            execute_never,
        }
    }
}

impl fmt::Display for AttributeFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mem_char = match self.mem_attributes {
            MemoryAttributes::CacheableDRAM => 'C',
        };

        let acc_str = match self.acc_perms {
            AccessPermissions::ReadWrite => "RW",
            AccessPermissions::ReadOnly => "RO",
        };

        let exec_str = if self.execute_never { "XN" } else { "X " };

        write!(f, "{} {} {}", mem_char, acc_str, exec_str)
    }
}
