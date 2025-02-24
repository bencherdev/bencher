use std::{fmt, iter::successors};

const BASE: u128 = 36;
const BUF_SIZE: usize = 25;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub struct Fingerprint(u128);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buffer = [0u8; BUF_SIZE];

        let divided = successors(Some(self.0), |n| match n / BASE {
            0 => None,

            n => Some(n),
        });

        let written = buffer
            .iter_mut()
            .rev()
            .zip(divided)
            .map(|(c, n)| *c = digit((n % BASE) as u8, f.alternate()))
            .count();

        let index = BUF_SIZE - written;
        let number = buffer.get(index..).unwrap_or_default();

        // There are only ASCII chars inside the buffer,
        // so the string should always be a valid UTF-8 string.
        let number_str = std::str::from_utf8(number).unwrap_or_default();

        f.write_str(number_str)
    }
}

#[inline]
fn digit(u: u8, alternate: bool) -> u8 {
    let a = if alternate { b'A' } else { b'a' };
    match u {
        0..=9 => u + b'0',
        10..=35 => u - 10 + a,
        _ => {
            debug_assert!(false, "Digit is greater than base {BASE}");
            b'0'
        },
    }
}

#[allow(clippy::unreadable_literal)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_display() {
        let fingerprint = Fingerprint(0x1234567890abcdef1234567890abcdef);
        let display = format!("{fingerprint}");
        assert_eq!(display, "12srde1xpfeunnr6aa7s0b4z3");

        let fingerprint = Fingerprint(0xffffffffffffffffffffffffffffffff);
        let display = format!("{fingerprint}");
        assert_eq!(display, "f5lxx1zz5pnorynqglhzmsp33");

        let fingerprint = Fingerprint(0);
        let display = format!("{fingerprint}");
        assert_eq!(display, "0");
    }

    #[test]
    fn test_fingerprint_display_alternate() {
        let fingerprint = Fingerprint(0x1234567890abcdef1234567890abcdef);
        let display = format!("{fingerprint:#}");
        assert_eq!(display, "12SRDE1XPFEUNNR6AA7S0B4Z3");

        let fingerprint = Fingerprint(0xffffffffffffffffffffffffffffffff);
        let display = format!("{fingerprint:#}");
        assert_eq!(display, "F5LXX1ZZ5PNORYNQGLHZMSP33");

        let fingerprint = Fingerprint(0);
        let display = format!("{fingerprint:#}");
        assert_eq!(display, "0");
    }
}
