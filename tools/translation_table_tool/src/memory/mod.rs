pub mod attributes;
pub mod region;

pub use attributes::*;
pub use region::*;

pub const GRANULE_64KIB_SIZE: usize = 64 * 1024;
pub const GRANULE_64KIB_SHIFT: usize = 16; // log2(64 * 1024)

pub const GRANULE_512MIB_SIZE: usize = 512 * 1024 * 1024;
pub const GRANULE_512MIB_SHIFT: usize = 29; // log2(512 * 1024 * 1024)
pub const GRANULE_512MIB_MASK: usize = GRANULE_512MIB_SIZE - 1;

pub trait Alignment {
    fn is_power_of_two(self) -> bool;
    fn is_aligned(self, alignment: Self) -> bool;
    fn align_up(self, alignment: Self) -> Self;
    fn to_hex_underscore(self, with_leading_zeros: bool) -> String;
}

impl Alignment for usize {
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    fn is_aligned(self, alignment: Self) -> bool {
        assert!(alignment.is_power_of_two());
        (self & (alignment - 1)) == 0
    }

    fn align_up(self, alignment: Self) -> Self {
        assert!(alignment.is_power_of_two());
        (self + alignment - 1) & !(alignment - 1)
    }

    fn to_hex_underscore(self, with_leading_zeros: bool) -> String {
        let hex_str = if with_leading_zeros {
            format!("{:016x}", self)
        } else {
            format!("{:x}", self)
        };

        let reversed: String = hex_str.chars().rev().collect();
        let chunked: Vec<String> = reversed
            .chars()
            .collect::<Vec<char>>()
            .chunks(4)
            .map(|chunk| chunk.iter().collect())
            .collect();

        let result: String = chunked.join("_");
        let final_result: String = result.chars().rev().collect();

        format!("0x{}", final_result)
    }
}

impl Alignment for u64 {
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }

    fn is_aligned(self, alignment: Self) -> bool {
        assert!(alignment.is_power_of_two());
        (self & (alignment - 1)) == 0
    }

    fn align_up(self, alignment: Self) -> Self {
        assert!(alignment.is_power_of_two());
        (self + alignment - 1) & !(alignment - 1)
    }

    fn to_hex_underscore(self, with_leading_zeros: bool) -> String {
        let hex_str = if with_leading_zeros {
            format!("{:016x}", self)
        } else {
            format!("{:x}", self)
        };

        let reversed: String = hex_str.chars().rev().collect();
        let chunked: Vec<String> = reversed
            .chars()
            .collect::<Vec<char>>()
            .chunks(4)
            .map(|chunk| chunk.iter().collect())
            .collect();

        let result: String = chunked.join("_");
        let final_result: String = result.chars().rev().collect();

        format!("0x{}", final_result)
    }
}
