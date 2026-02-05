//! Unit conversion helpers for memory/disk sizes.
//!
//! The API sends memory and disk sizes in bytes, but Firecracker expects MiB.
//! These helpers convert between the two representations.

/// Bytes per MiB (1024 * 1024 = 1048576).
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// Convert bytes to MiB, rounding up.
///
/// This is used when converting API values (bytes) to Firecracker values (MiB).
/// Rounding up ensures we never under-allocate resources.
///
/// # Examples
///
/// ```
/// use bencher_runner::units::bytes_to_mib;
///
/// assert_eq!(bytes_to_mib(0), 0);
/// assert_eq!(bytes_to_mib(1), 1);  // 1 byte rounds up to 1 MiB
/// assert_eq!(bytes_to_mib(1048576), 1);  // Exactly 1 MiB
/// assert_eq!(bytes_to_mib(1048577), 2);  // 1 MiB + 1 byte rounds up to 2 MiB
/// assert_eq!(bytes_to_mib(1073741824), 1024);  // 1 GiB = 1024 MiB
/// ```
#[must_use]
pub const fn bytes_to_mib(bytes: u64) -> u32 {
    // Ceiling division using div_ceil
    // But we need to handle bytes == 0 specially
    if bytes == 0 {
        0
    } else {
        let mib = bytes.div_ceil(BYTES_PER_MIB);
        // Saturate to u32::MAX if the result doesn't fit
        // Use cast instead of From trait since From isn't const
        if mib > u32::MAX as u64 {
            u32::MAX
        } else {
            // This cast is safe because we just checked that mib <= u32::MAX
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Checked that value fits in u32 above"
            )]
            let result = mib as u32;
            result
        }
    }
}

/// Convert MiB to bytes.
///
/// This is used when converting Firecracker values (MiB) to API values (bytes).
///
/// # Examples
///
/// ```
/// use bencher_runner::units::mib_to_bytes;
///
/// assert_eq!(mib_to_bytes(0), 0);
/// assert_eq!(mib_to_bytes(1), 1048576);  // 1 MiB
/// assert_eq!(mib_to_bytes(1024), 1073741824);  // 1 GiB
/// ```
#[must_use]
pub const fn mib_to_bytes(mib: u32) -> u64 {
    // Use cast instead of From trait since From isn't const
    (mib as u64) * BYTES_PER_MIB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_mib_zero() {
        assert_eq!(bytes_to_mib(0), 0);
    }

    #[test]
    fn bytes_to_mib_exact() {
        assert_eq!(bytes_to_mib(BYTES_PER_MIB), 1);
        assert_eq!(bytes_to_mib(2 * BYTES_PER_MIB), 2);
        assert_eq!(bytes_to_mib(1024 * BYTES_PER_MIB), 1024);
    }

    #[test]
    fn bytes_to_mib_rounds_up() {
        assert_eq!(bytes_to_mib(1), 1);
        assert_eq!(bytes_to_mib(BYTES_PER_MIB - 1), 1);
        assert_eq!(bytes_to_mib(BYTES_PER_MIB + 1), 2);
        assert_eq!(bytes_to_mib(2 * BYTES_PER_MIB + 1), 3);
    }

    #[test]
    fn bytes_to_mib_large_values() {
        // 10 GiB = 10 * 1024 MiB
        assert_eq!(bytes_to_mib(10 * 1024 * BYTES_PER_MIB), 10 * 1024);
        // 100 GiB
        assert_eq!(bytes_to_mib(100 * 1024 * BYTES_PER_MIB), 100 * 1024);
    }

    #[test]
    fn mib_to_bytes_zero() {
        assert_eq!(mib_to_bytes(0), 0);
    }

    #[test]
    fn mib_to_bytes_exact() {
        assert_eq!(mib_to_bytes(1), BYTES_PER_MIB);
        assert_eq!(mib_to_bytes(2), 2 * BYTES_PER_MIB);
        assert_eq!(mib_to_bytes(1024), 1024 * BYTES_PER_MIB);
    }

    #[test]
    fn roundtrip() {
        // Exact MiB values should round-trip perfectly
        for mib in [0, 1, 2, 512, 1024, 2048] {
            assert_eq!(bytes_to_mib(mib_to_bytes(mib)), mib);
        }
    }

    #[test]
    fn mib_to_bytes_max() {
        // u32::MAX MiB should not overflow u64
        let max_bytes = mib_to_bytes(u32::MAX);
        assert_eq!(max_bytes, u64::from(u32::MAX) * BYTES_PER_MIB);
    }
}
