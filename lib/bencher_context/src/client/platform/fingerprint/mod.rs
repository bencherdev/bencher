use std::fmt;

use bencher_valid::BASE_36;
use uuid::Uuid;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy)]
pub struct Fingerprint(Uuid);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", encode_uuid(self.0))
    }
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
impl Fingerprint {
    #[allow(clippy::unnecessary_wraps)]
    pub fn current() -> Option<Self> {
        None
    }
}

// The maximum number of characters for a base 36 encoded u64 is 13.
const ENCODED_LEN: usize = 13;
fn encode_uuid(uuid: Uuid) -> String {
    let base = BASE_36.len() as u64;
    let chars = BASE_36.chars().collect::<Vec<_>>();

    let (lhs, rhs) = uuid.as_u64_pair();
    let mut num = hash_combined(lhs, rhs);

    let mut result = String::new();
    while num > 0 {
        #[allow(clippy::cast_possible_truncation)]
        let remainder = (num % base) as usize;
        if let Some(c) = chars.get(remainder) {
            result.push(*c);
        }
        num /= base;
    }

    result
        .chars()
        .chain(std::iter::repeat('0'))
        .take(ENCODED_LEN)
        .collect()
}

// https://stackoverflow.com/a/27952689
// https://www.boost.org/doc/libs/1_43_0/doc/html/hash/reference.html#boost.hash_combine
#[allow(clippy::unreadable_literal)]
const GOLDEN_RATIO: u64 = 0x9e3779b97f4a7c15;
fn hash_combined(lhs: u64, rhs: u64) -> u64 {
    lhs ^ (rhs
        .wrapping_add(GOLDEN_RATIO)
        .wrapping_add(lhs << 6)
        .wrapping_add(lhs >> 2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_encode_uuid() {
        let uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        assert_eq!(encode_uuid(uuid), "p78ezm4408me2");

        let uuid = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        assert_eq!(encode_uuid(uuid), "gf47mznnithi0");

        let uuid = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        assert_eq!(encode_uuid(uuid), "nfi03cu0p7x71");
    }
}
