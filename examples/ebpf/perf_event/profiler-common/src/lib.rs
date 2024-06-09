#![no_std]

use core::mem;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sample {
    pub header: SampleHeader,
    pub stack: [u8; Self::STACK_SIZE],
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            header: SampleHeader::default(),
            stack: [0; Self::STACK_SIZE],
        }
    }
}

impl Sample {
    pub const STACK_SIZE: usize = 4_096;
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for Sample {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct SampleHeader {
    pub ktime: u64,
    pub pid: u32,
    pub tid: u32,
    pub stack_len: u64,
}

impl SampleHeader {
    pub const SIZE: usize = mem::size_of::<Self>();
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for SampleHeader {}
