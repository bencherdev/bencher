#![no_std]

#[repr(C)]
#[derive(Clone, Copy)]
#[cfg_attr(feature = "user", derive(Debug))]
pub enum SourceAddr {
    Fizz,
    Buzz,
    FizzBuzz,
    Fibonacci,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for SourceAddr {}
