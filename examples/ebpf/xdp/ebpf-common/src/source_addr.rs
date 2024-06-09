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

#[cfg(not(feature = "user"))]
mod ebpf {
    use super::SourceAddr;

    impl SourceAddr {
        pub fn new_v1(source_addr: u32) -> Option<SourceAddr> {
            (source_addr % 3 == 0).then_some(SourceAddr::Fizz)
        }

        pub fn new_v2(source_addr: u32) -> Option<SourceAddr> {
            match (source_addr % 3, source_addr % 5) {
                (0, 0) => Some(SourceAddr::FizzBuzz),
                (0, _) => Some(SourceAddr::Fizz),
                (_, 0) => Some(SourceAddr::Buzz),
                _ => None,
            }
        }

        pub fn new_v3(source_addr: u32) -> Option<SourceAddr> {
            is_fibonacci(source_addr as u8)
                .then_some(SourceAddr::Fibonacci)
                .or(match (source_addr % 3, source_addr % 5) {
                    (0, 0) => Some(SourceAddr::FizzBuzz),
                    (0, _) => Some(SourceAddr::Fizz),
                    (_, 0) => Some(SourceAddr::Buzz),
                    _ => None,
                })
        }

        pub fn new_v4(source_addr: u32) -> Option<SourceAddr> {
            is_fibonacci_memo(source_addr as u8)
                .then_some(SourceAddr::Fibonacci)
                .or(match (source_addr % 3, source_addr % 5) {
                    (0, 0) => Some(SourceAddr::FizzBuzz),
                    (0, _) => Some(SourceAddr::Fizz),
                    (_, 0) => Some(SourceAddr::Buzz),
                    _ => None,
                })
        }
    }

    fn is_fibonacci(n: u8) -> bool {
        let (mut a, mut b) = (0, 1);
        while b < n {
            let c = a + b;
            a = b;
            b = c;
        }
        b == n
    }

    fn is_fibonacci_memo(n: u8) -> bool {
        matches!(
            n,
            0 | 1 | 2 | 3 | 5 | 8 | 13 | 21 | 34 | 55 | 89 | 144 | 233
        )
    }
}
