//! Raw WAL bytes built from scratch, with an independent implementation of
//! the `SQLite` WAL checksum chain (double-entry bookkeeping against
//! [`crate::wal::wal_checksum`]).
//!
//! Covers inputs rusqlite on a little-endian host can never produce:
//! big-endian checksum order, stale salts, bit flips, torn files.

/// Builder for synthetic WAL byte strings.
pub struct SyntheticWal {
    bytes: Vec<u8>,
    page_size: u32,
    big_endian: bool,
    salt: (u32, u32),
    running: (u32, u32),
}

impl SyntheticWal {
    /// Start a WAL with the given page size and checksum byte order
    /// (`big_endian` true selects magic `0x377f0683`).
    #[must_use]
    pub fn new(page_size: u32, big_endian: bool, salt: (u32, u32)) -> Self {
        // Independent literals on purpose: reusing crate::wal constants here
        // would defeat the double-entry bookkeeping.
        let magic = if big_endian { 0x377f_0683 } else { 0x377f_0682 };
        let mut bytes = Vec::with_capacity(32);
        push_be_u32(&mut bytes, magic);
        push_be_u32(&mut bytes, 3_007_000);
        push_be_u32(&mut bytes, page_size);
        push_be_u32(&mut bytes, 0); // checkpoint sequence number
        push_be_u32(&mut bytes, salt.0);
        push_be_u32(&mut bytes, salt.1);
        // Header self-checksum over bytes 0..24, seeded with (0, 0)
        let running = fold_checksum(big_endian, (0, 0), &bytes);
        push_be_u32(&mut bytes, running.0);
        push_be_u32(&mut bytes, running.1);
        Self {
            bytes,
            page_size,
            big_endian,
            salt,
            running,
        }
    }

    /// Append a non-commit frame for `page_no` with the checksum chain
    /// threaded automatically.
    #[must_use]
    pub fn frame(self, page_no: u32, page: &[u8]) -> Self {
        let salt = self.salt;
        self.push_frame(page_no, page, 0, salt)
    }

    /// Append a commit frame (`db_size` pages).
    #[must_use]
    pub fn commit_frame(self, page_no: u32, page: &[u8], db_size: u32) -> Self {
        let salt = self.salt;
        self.push_frame(page_no, page, db_size, salt)
    }

    /// Append a frame carrying explicit (stale) salts, simulating leftovers
    /// from before a WAL restart.
    ///
    /// The checksum chain is still threaded normally, so the mismatched salts
    /// are the frame's only defect.
    #[must_use]
    pub fn frame_with_salts(self, page_no: u32, page: &[u8], salt: (u32, u32)) -> Self {
        self.push_frame(page_no, page, 0, salt)
    }

    /// Flip one bit at `byte_offset` (corruption injection).
    #[must_use]
    pub fn flip_bit(mut self, byte_offset: usize, bit: u8) -> Self {
        assert!(bit < 8, "bit index must be in 0..8, got {bit}");
        assert!(
            byte_offset < self.bytes.len(),
            "flip_bit offset {byte_offset} out of range for {} bytes",
            self.bytes.len()
        );
        if let Some(byte) = self.bytes.get_mut(byte_offset) {
            *byte ^= 1u8 << bit;
        }
        self
    }

    /// Truncate the byte string to `len` (torn header / mid-frame EOF).
    /// A `len` beyond the current size leaves the bytes unchanged.
    #[must_use]
    pub fn truncate_at(mut self, len: usize) -> Self {
        self.bytes.truncate(len);
        self
    }

    /// The finished WAL bytes.
    #[must_use]
    pub fn bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn push_frame(mut self, page_no: u32, page: &[u8], db_size: u32, salt: (u32, u32)) -> Self {
        assert_eq!(
            page.len(),
            self.page_size as usize,
            "page payload must be exactly page_size bytes"
        );
        // The frame checksum covers frame-header bytes 0..8 plus the page,
        // seeded from the previous frame (or the WAL header)
        let mut head = Vec::with_capacity(24);
        push_be_u32(&mut head, page_no);
        push_be_u32(&mut head, db_size);
        let checksum = fold_checksum(self.big_endian, self.running, &head);
        let checksum = fold_checksum(self.big_endian, checksum, page);
        push_be_u32(&mut head, salt.0);
        push_be_u32(&mut head, salt.1);
        push_be_u32(&mut head, checksum.0);
        push_be_u32(&mut head, checksum.1);
        self.bytes.extend_from_slice(&head);
        self.bytes.extend_from_slice(page);
        self.running = checksum;
        self
    }
}

/// Append a `u32` in big-endian byte order (all WAL integer fields, most
/// significant byte first). Each extracted byte is masked to 8 bits, so the
/// cast is exact; the assert surfaces a logic error loudly instead of
/// silently writing a zero byte if that masking is ever broken.
fn push_be_u32(bytes: &mut Vec<u8>, value: u32) {
    for shift in [24u32, 16, 8, 0] {
        let byte = (value >> shift) & 0xff;
        let narrow = byte as u8;
        // Masked to 8 bits, so the round-trip is exact; assert loudly rather
        // than silently truncating if that masking is ever broken.
        assert_eq!(u32::from(narrow), byte, "WAL byte {byte:#x} must fit in u8");
        bytes.push(narrow);
    }
}

/// Independent implementation of `SQLite`'s cumulative WAL checksum: the input
/// is folded as pairs of 32-bit words in the given byte order.
fn fold_checksum(big_endian: bool, seed: (u32, u32), data: &[u8]) -> (u32, u32) {
    assert!(
        data.len().is_multiple_of(8),
        "checksum input must be a multiple of 8 bytes, got {}",
        data.len()
    );
    let mut sums = seed;
    for pair in data.chunks_exact(8) {
        let (first, second) = pair.split_at(4);
        let x0 = word(big_endian, first);
        let x1 = word(big_endian, second);
        sums.0 = sums.0.wrapping_add(x0).wrapping_add(sums.1);
        sums.1 = sums.1.wrapping_add(x1).wrapping_add(sums.0);
    }
    sums
}

/// Read one 32-bit checksum word in the given byte order.
fn word(big_endian: bool, bytes: &[u8]) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(bytes);
    if big_endian {
        (u32::from(buf[0]) << 24)
            | (u32::from(buf[1]) << 16)
            | (u32::from(buf[2]) << 8)
            | u32::from(buf[3])
    } else {
        u32::from(buf[0])
            | (u32::from(buf[1]) << 8)
            | (u32::from(buf[2]) << 16)
            | (u32::from(buf[3]) << 24)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::wal::{
        FrameHeader, WAL_FORMAT, WAL_MAGIC_BE, WAL_MAGIC_LE, parse_wal_header, wal_checksum,
    };

    const PAGE: u32 = 512;
    const SALT: (u32, u32) = (0xdead_beef, 0xcafe_f00d);

    fn page_of(fill: u8) -> Vec<u8> {
        vec![fill; usize::try_from(PAGE).unwrap()]
    }

    #[test]
    fn header_fields_roundtrip_both_orders() {
        for (big_endian, magic) in [(false, WAL_MAGIC_LE), (true, WAL_MAGIC_BE)] {
            let bytes = SyntheticWal::new(PAGE, big_endian, SALT).bytes();
            assert_eq!(bytes.len(), 32, "header only");
            let header = parse_wal_header(&bytes).unwrap();
            assert_eq!(header.magic, magic);
            assert_eq!(header.format, WAL_FORMAT);
            assert_eq!(header.page_size, PAGE);
            assert_eq!(header.checkpoint_seq, 0);
            assert_eq!(header.salt, SALT);
            // The self-checksum verifies under the crate implementation
            let computed = wal_checksum(big_endian, (0, 0), &bytes[..24]);
            assert_eq!(header.checksum, computed);
        }
    }

    #[test]
    fn frame_checksums_match_crate_checksum() {
        // Double-entry bookkeeping: the builder's independent checksum chain
        // must agree with crate::wal::wal_checksum frame by frame.
        for big_endian in [false, true] {
            let bytes = SyntheticWal::new(PAGE, big_endian, SALT)
                .frame(1, &page_of(0x11))
                .commit_frame(2, &page_of(0x22), 2)
                .bytes();
            let header = parse_wal_header(&bytes).unwrap();
            let frame_size = 24 + usize::try_from(PAGE).unwrap();
            let mut running = header.checksum;
            for frame in bytes[32..].chunks_exact(frame_size) {
                let mut head = [0u8; 24];
                head.copy_from_slice(&frame[..24]);
                let frame_header = FrameHeader::parse(&head);
                assert_eq!(frame_header.salt, SALT, "frames carry the header salts");
                let after_head = wal_checksum(big_endian, running, &frame[..8]);
                let computed = wal_checksum(big_endian, after_head, &frame[24..]);
                assert_eq!(
                    frame_header.checksum, computed,
                    "stored frame checksum matches the crate chain (big_endian: {big_endian})"
                );
                running = computed;
            }
        }
    }

    #[test]
    fn commit_frame_records_db_size() {
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .frame(1, &page_of(0x11))
            .commit_frame(2, &page_of(0x22), 9)
            .bytes();
        let frame_size = 24 + usize::try_from(PAGE).unwrap();
        let mut head = [0u8; 24];
        head.copy_from_slice(&bytes[32..32 + 24]);
        let first = FrameHeader::parse(&head);
        assert_eq!(first.page_no, 1);
        assert_eq!(first.db_size, 0, "non-commit frame");
        assert!(!first.is_commit());
        head.copy_from_slice(&bytes[32 + frame_size..32 + frame_size + 24]);
        let second = FrameHeader::parse(&head);
        assert_eq!(second.page_no, 2);
        assert_eq!(second.db_size, 9, "commit frame carries the db size");
        assert!(second.is_commit());
    }

    #[test]
    fn frame_with_salts_uses_given_salts() {
        let stale = (0x0101_0101, 0x0202_0202);
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .frame_with_salts(1, &page_of(0x11), stale)
            .bytes();
        let mut head = [0u8; 24];
        head.copy_from_slice(&bytes[32..32 + 24]);
        let frame = FrameHeader::parse(&head);
        assert_eq!(frame.salt, stale, "explicit salts are written verbatim");
    }

    #[test]
    fn flip_bit_and_truncate_mutate_bytes() {
        let pristine = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0x11), 1)
            .bytes();
        let flipped = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0x11), 1)
            .flip_bit(40, 3)
            .bytes();
        assert_eq!(pristine.len(), flipped.len());
        assert_eq!(
            pristine[40] ^ (1 << 3),
            flipped[40],
            "exactly one bit flips"
        );
        let differing = pristine
            .iter()
            .zip(&flipped)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(differing, 1, "no other byte changes");

        let truncated = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0x11), 1)
            .truncate_at(100)
            .bytes();
        assert_eq!(truncated.len(), 100);
        assert_eq!(&truncated[..], &pristine[..100]);
    }
}
