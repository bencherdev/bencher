//! `SQLite` WAL file parsing: header and frame validation with salt and
//! cumulative checksum verification.
//!
//! Format reference: <https://www.sqlite.org/walformat.html>
//!
//! Layout: a 32-byte header followed by zero or more frames. Each frame is a
//! 24-byte header plus one page of payload (`page_size` bytes).
//!
//! Only bytes up through the last *commit* frame may ever be shipped to the
//! replica: frames after the last commit belong to an open (or torn)
//! transaction and are not durable.

use std::io::{Read, Seek, SeekFrom};

/// Size of the WAL file header in bytes.
pub const WAL_HEADER_SIZE: u64 = 32;
/// Size of each frame header in bytes.
pub const FRAME_HEADER_SIZE: u64 = 24;

/// [`WAL_HEADER_SIZE`] as a `usize` for buffer sizing.
const WAL_HEADER_LEN: usize = 32;
/// [`FRAME_HEADER_SIZE`] as a `usize` for buffer sizing.
const FRAME_HEADER_LEN: usize = 24;
/// Minimum valid `SQLite` page size.
const MIN_PAGE_SIZE: u32 = 512;
/// Maximum valid `SQLite` page size (65536).
const MAX_PAGE_SIZE: u32 = 0x0001_0000;

/// WAL magic when checksums are computed over little-endian words.
pub const WAL_MAGIC_LE: u32 = 0x377f_0682;
/// WAL magic when checksums are computed over big-endian words.
pub const WAL_MAGIC_BE: u32 = 0x377f_0683;
/// The only supported WAL format version.
pub const WAL_FORMAT: u32 = 3_007_000;

/// Parsed WAL file header (all fields stored big-endian on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalHeader {
    pub magic: u32,
    pub format: u32,
    pub page_size: u32,
    pub checkpoint_seq: u32,
    pub salt: (u32, u32),
    /// Header self-checksum over the first 24 bytes, seeded with (0, 0).
    pub checksum: (u32, u32),
}

impl WalHeader {
    /// Whether frame checksums are computed over big-endian 32-bit words
    /// (magic `0x377f0683`) or little-endian words (magic `0x377f0682`).
    #[must_use]
    pub fn big_endian_checksum(&self) -> bool {
        self.magic == WAL_MAGIC_BE
    }

    /// Total on-disk size of one frame: header plus one page.
    #[must_use]
    pub fn frame_size(&self) -> u64 {
        FRAME_HEADER_SIZE + u64::from(self.page_size)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("Failed to read WAL: {0}")]
    Read(#[source] std::io::Error),
    #[error("Failed to seek WAL: {0}")]
    Seek(#[source] std::io::Error),
    #[error("WAL header is truncated: {0} bytes")]
    TruncatedHeader(usize),
    #[error("Invalid WAL magic: {0:#010x}")]
    BadMagic(u32),
    #[error("Unsupported WAL format version: {0}")]
    UnsupportedFormat(u32),
    #[error("Invalid WAL page size: {0}")]
    InvalidPageSize(u32),
    #[error("WAL header checksum mismatch: stored {stored:?}, computed {computed:?}")]
    HeaderChecksum {
        stored: (u32, u32),
        computed: (u32, u32),
    },
    #[error(
        "WAL resume position is invalid: offset {offset} is not frame-aligned for page size {page_size}"
    )]
    MisalignedOffset { offset: u64, page_size: u32 },
    #[error(
        "A WAL run of {bytes} bytes since the last commit exceeds the scan bound of {max_bytes}; refusing to buffer or ship it"
    )]
    TransactionTooLarge { bytes: u64, max_bytes: u64 },
}

/// Parse and validate a 32-byte WAL header.
///
/// Errors on truncation, bad magic, unsupported format, invalid page size
/// (must be a power of two between 512 and 65536), or a header self-checksum
/// mismatch.
pub fn parse_wal_header(bytes: &[u8]) -> Result<WalHeader, WalError> {
    let header: &[u8; WAL_HEADER_LEN] = bytes
        .get(..WAL_HEADER_LEN)
        .and_then(|prefix| prefix.try_into().ok())
        .ok_or(WalError::TruncatedHeader(bytes.len()))?;
    let magic = be_u32(header[0], header[1], header[2], header[3]);
    if magic != WAL_MAGIC_LE && magic != WAL_MAGIC_BE {
        return Err(WalError::BadMagic(magic));
    }
    let format = be_u32(header[4], header[5], header[6], header[7]);
    if format != WAL_FORMAT {
        return Err(WalError::UnsupportedFormat(format));
    }
    let page_size = be_u32(header[8], header[9], header[10], header[11]);
    if !page_size.is_power_of_two() || !(MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err(WalError::InvalidPageSize(page_size));
    }
    let checkpoint_seq = be_u32(header[12], header[13], header[14], header[15]);
    let salt = (
        be_u32(header[16], header[17], header[18], header[19]),
        be_u32(header[20], header[21], header[22], header[23]),
    );
    let stored = (
        be_u32(header[24], header[25], header[26], header[27]),
        be_u32(header[28], header[29], header[30], header[31]),
    );
    // The self-checksum covers header bytes 0..24 (everything before it)
    let (covered, _stored_bytes) = header.split_at(24);
    let computed = wal_checksum(magic == WAL_MAGIC_BE, (0, 0), covered);
    if stored != computed {
        return Err(WalError::HeaderChecksum { stored, computed });
    }
    Ok(WalHeader {
        magic,
        format,
        page_size,
        checkpoint_seq,
        salt,
        checksum: stored,
    })
}

/// `SQLite`'s custom cumulative WAL checksum.
///
/// `data.len()` must be a multiple of 8. Words are read as big-endian when
/// `big_endian` is true, little-endian otherwise. The running pair `seed`
/// (`s1`, `s2`) is folded as: `s1 += x[i] + s2; s2 += x[i+1] + s1;` with
/// wrapping 32-bit arithmetic.
///
/// - Header self-checksum: input is header bytes `0..24`, seed `(0, 0)`.
/// - Frame checksum: input is frame-header bytes `0..8` followed by the full
///   page payload, seeded with the previous frame's checksum (or the header
///   checksum for the first frame). Stored in frame-header bytes `16..24`.
#[must_use]
pub fn wal_checksum(big_endian: bool, seed: (u32, u32), data: &[u8]) -> (u32, u32) {
    assert!(
        data.len().is_multiple_of(8),
        "WAL checksum input must be a multiple of 8 bytes"
    );
    let (mut s1, mut s2) = seed;
    for pair in data.chunks_exact(8) {
        let mut word = [0u8; 8];
        word.copy_from_slice(pair);
        let (x0, x1) = if big_endian {
            (
                be_u32(word[0], word[1], word[2], word[3]),
                be_u32(word[4], word[5], word[6], word[7]),
            )
        } else {
            (
                le_u32(word[0], word[1], word[2], word[3]),
                le_u32(word[4], word[5], word[6], word[7]),
            )
        };
        s1 = s1.wrapping_add(x0).wrapping_add(s2);
        s2 = s2.wrapping_add(x1).wrapping_add(s1);
    }
    (s1, s2)
}

/// Parsed 24-byte frame header (all fields big-endian on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub page_no: u32,
    /// For commit frames, the size of the database in pages after the commit;
    /// zero for all non-commit frames.
    pub db_size: u32,
    pub salt: (u32, u32),
    pub checksum: (u32, u32),
}

impl FrameHeader {
    /// Extract the fields from a raw 24-byte frame header.
    #[must_use]
    pub fn parse(bytes: &[u8; 24]) -> Self {
        Self {
            page_no: be_u32(bytes[0], bytes[1], bytes[2], bytes[3]),
            db_size: be_u32(bytes[4], bytes[5], bytes[6], bytes[7]),
            salt: (
                be_u32(bytes[8], bytes[9], bytes[10], bytes[11]),
                be_u32(bytes[12], bytes[13], bytes[14], bytes[15]),
            ),
            checksum: (
                be_u32(bytes[16], bytes[17], bytes[18], bytes[19]),
                be_u32(bytes[20], bytes[21], bytes[22], bytes[23]),
            ),
        }
    }

    /// Whether this frame commits a transaction.
    #[must_use]
    pub fn is_commit(&self) -> bool {
        self.db_size != 0
    }
}

/// A run of verified, committed WAL bytes ending exactly on a commit frame.
#[derive(Debug, Clone)]
pub struct CommittedChunk {
    /// Byte offset in the WAL file where this chunk starts (inclusive).
    pub start_offset: u64,
    /// Byte offset just past the last commit frame in this chunk (exclusive).
    pub end_offset: u64,
    /// Running checksum at `end_offset`, for resuming the chain.
    pub checksum_at_end: (u32, u32),
    /// Number of commit frames contained in this chunk.
    pub commit_count: u64,
    /// Database size in pages recorded by the last commit frame.
    pub db_size_pages: u32,
    /// Raw WAL bytes `[start_offset, end_offset)`.
    pub bytes: Vec<u8>,
}

/// Incremental, checksum-verified scanner over a WAL file.
///
/// The scanner walks frames from a known position, verifying that each
/// frame's salts match the header salts and that the cumulative checksum
/// chain is intact. It stops (returning `None`) at the first frame that is
/// torn, checksum-broken, salt-stale, pgno-0, or past EOF; frames after the
/// last commit are never surfaced. A stop is not an error: the valid prefix
/// up to the stop point remains shippable.
pub struct WalScanner<R> {
    reader: R,
    header: WalHeader,
    /// Next unread byte offset; always frame-aligned:
    /// `(offset - 32) % frame_size == 0`.
    offset: u64,
    /// Running checksum at `offset`.
    checksum: (u32, u32),
}

impl<R: Read + Seek> WalScanner<R> {
    /// Open a WAL from the start: read and validate the header, position the
    /// scanner at the first frame.
    ///
    /// Returns `Ok(None)` when the file is shorter than a full header (an
    /// empty or freshly truncated WAL): there is nothing to ship and nothing
    /// wrong.
    pub fn open(mut reader: R) -> Result<Option<Self>, WalError> {
        reader.seek(SeekFrom::Start(0)).map_err(WalError::Seek)?;
        let mut header_buf = [0u8; WAL_HEADER_LEN];
        if !read_full(&mut reader, &mut header_buf)? {
            return Ok(None);
        }
        let header = parse_wal_header(&header_buf)?;
        Ok(Some(Self {
            reader,
            header,
            offset: WAL_HEADER_SIZE,
            checksum: header.checksum,
        }))
    }

    /// Resume scanning from a previously verified position.
    ///
    /// `offset` must be frame-aligned and `checksum` must be the running
    /// checksum at that offset (as previously returned in
    /// [`CommittedChunk::checksum_at_end`]).
    pub fn resume(
        reader: R,
        header: WalHeader,
        offset: u64,
        checksum: (u32, u32),
    ) -> Result<Self, WalError> {
        let aligned = offset
            .checked_sub(WAL_HEADER_SIZE)
            .is_some_and(|frame_bytes| frame_bytes.is_multiple_of(header.frame_size()));
        if !aligned {
            return Err(WalError::MisalignedOffset {
                offset,
                page_size: header.page_size,
            });
        }
        Ok(Self {
            reader,
            header,
            offset,
            checksum,
        })
    }

    /// The validated WAL header.
    #[must_use]
    pub fn header(&self) -> &WalHeader {
        &self.header
    }

    /// Current frame-aligned offset (next unread byte).
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Running checksum at [`Self::offset`].
    #[must_use]
    pub fn checksum(&self) -> (u32, u32) {
        self.checksum
    }

    /// Read the next run of committed frames, up to `max_bytes` of raw WAL
    /// bytes (always ending on a commit-frame boundary; a single transaction
    /// larger than `max_bytes` is returned whole, so chunks may exceed
    /// `max_bytes`).
    ///
    /// Returns `Ok(None)` when no complete committed transaction lies beyond
    /// the current offset (clean EOF, torn tail, checksum break, or stale
    /// salts). The scanner does not advance past the last commit it returns.
    ///
    /// MEMORY: this retaining scan applies no transaction bound (returning an
    /// oversized transaction whole is the contract), so an arbitrarily large
    /// open or rolled-back tail is heap-buffered before being discarded.
    /// Callers facing an untrusted tail must front-run with the bounded
    /// [`Self::scan_committed_extent`] (as the ship path does) and cap this
    /// read to the discovered extent.
    pub fn next_committed(&mut self, max_bytes: u64) -> Result<Option<CommittedChunk>, WalError> {
        // Retain page bytes; no transaction bound (the public contract is to
        // return an oversized transaction whole).
        self.scan(max_bytes, u64::MAX, Retention::Retain)
    }

    /// Scan-only variant of [`Self::next_committed`]: validate the committed
    /// prefix and advance to the last commit within `max_bytes` while
    /// DISCARDING page bytes (checksums require reading every byte, not
    /// retaining it). `max_txn_bytes` bounds the bytes read since the last
    /// commit, so an oversized open transaction errors with
    /// [`WalError::TransactionTooLarge`] instead of scanning (and, for the
    /// retaining scan, allocating) without bound. Returns the absolute WAL
    /// offset just past the last commit (equal to [`Self::offset`] on entry
    /// when nothing new is committed).
    pub fn scan_committed_extent(
        &mut self,
        max_bytes: u64,
        max_txn_bytes: u64,
    ) -> Result<u64, WalError> {
        self.scan(max_bytes, max_txn_bytes, Retention::Discard)?;
        Ok(self.offset)
    }

    /// The shared scan loop behind [`Self::next_committed`] and
    /// [`Self::scan_committed_extent`]. `retention` selects whether page
    /// bytes are accumulated (a heap buffer for the returned chunk) or
    /// discarded (offsets and checksums only). `max_txn_bytes` is the bytes
    /// permitted since the last commit before the scan aborts, so a huge
    /// open transaction can never buffer or scan unboundedly.
    fn scan(
        &mut self,
        max_bytes: u64,
        max_txn_bytes: u64,
        retention: Retention,
    ) -> Result<Option<CommittedChunk>, WalError> {
        self.reader
            .seek(SeekFrom::Start(self.offset))
            .map_err(WalError::Seek)?;
        let frame_size = self.header.frame_size();
        let big_endian = self.header.big_endian_checksum();
        let mut frame_header_buf = [0u8; FRAME_HEADER_LEN];
        let mut page_buf = vec![0u8; self.header.page_size as usize];
        let retain = matches!(retention, Retention::Retain);

        // Frames read so far, possibly extending past the last commit (empty
        // in discard mode).
        let mut pending: Vec<u8> = Vec::new();
        let mut cursor = self.offset;
        let mut running = self.checksum;

        // High-water mark of the last commit frame seen
        let mut committed_len = 0usize;
        let mut committed_end = self.offset;
        let mut committed_checksum = self.checksum;
        let mut commit_count = 0u64;
        let mut db_size_pages = 0u32;

        loop {
            // A torn read anywhere in the frame ends the valid prefix
            if !read_full(&mut self.reader, &mut frame_header_buf)?
                || !read_full(&mut self.reader, &mut page_buf)?
            {
                break;
            }
            let frame = FrameHeader::parse(&frame_header_buf);
            // Stale salts (leftovers from before a WAL restart) end the prefix
            if frame.salt != self.header.salt {
                break;
            }
            // `SQLite`'s walDecodeFrame rejects page number 0; a checksum-valid
            // pgno-0 frame is not a real frame and ends the valid prefix
            // exactly like a checksum mismatch.
            if frame.page_no == 0 {
                break;
            }
            // The frame checksum covers frame-header bytes 0..8 plus the page,
            // seeded from the previous frame (or the WAL header)
            let (covered_head, _rest) = frame_header_buf.split_at(8);
            let after_head = wal_checksum(big_endian, running, covered_head);
            let computed = wal_checksum(big_endian, after_head, &page_buf);
            if computed != frame.checksum {
                break;
            }
            running = computed;
            if retain {
                pending.extend_from_slice(&frame_header_buf);
                pending.extend_from_slice(&page_buf);
            }
            cursor += frame_size;
            // Bound the run since the last commit BEFORE it grows further, so
            // an open (or committed) transaction beyond the cap aborts here
            // instead of buffering or scanning the whole thing.
            let since_commit = cursor.saturating_sub(committed_end);
            if since_commit > max_txn_bytes {
                return Err(WalError::TransactionTooLarge {
                    bytes: since_commit,
                    max_bytes: max_txn_bytes,
                });
            }
            if frame.is_commit() {
                committed_len = pending.len();
                committed_end = cursor;
                committed_checksum = running;
                commit_count += 1;
                db_size_pages = frame.db_size;
                if committed_end - self.offset >= max_bytes {
                    break;
                }
            }
        }

        if commit_count == 0 {
            return Ok(None);
        }
        // Drop any valid-but-uncommitted frames read past the last commit
        // (a no-op in discard mode, where `pending` stayed empty).
        pending.truncate(committed_len);
        let chunk = CommittedChunk {
            start_offset: self.offset,
            end_offset: committed_end,
            checksum_at_end: committed_checksum,
            commit_count,
            db_size_pages,
            bytes: pending,
        };
        self.offset = committed_end;
        self.checksum = committed_checksum;
        Ok(Some(chunk))
    }
}

/// Whether [`WalScanner::scan`] retains page bytes for a returned chunk or
/// discards them (offsets and checksums only).
#[derive(Clone, Copy)]
enum Retention {
    Retain,
    Discard,
}

/// Fill `buf` from `reader`. Returns `Ok(true)` when completely filled,
/// `Ok(false)` on a clean or torn EOF before the buffer is full.
fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool, WalError> {
    let mut filled = 0usize;
    while filled < buf.len() {
        let Some(rest) = buf.get_mut(filled..) else {
            break;
        };
        match reader.read(rest) {
            Ok(0) => return Ok(false),
            Ok(n) => filled += n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {},
            Err(err) => return Err(WalError::Read(err)),
        }
    }
    Ok(true)
}

/// Assemble a big-endian `u32` (`SQLite` WAL integer fields are big-endian).
fn be_u32(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    (u32::from(b0) << 24) | (u32::from(b1) << 16) | (u32::from(b2) << 8) | u32::from(b3)
}

/// Assemble a little-endian `u32` (checksum words under magic `0x377f0682`).
fn le_u32(b0: u8, b1: u8, b2: u8, b3: u8) -> u32 {
    u32::from(b0) | (u32::from(b1) << 8) | (u32::from(b2) << 16) | (u32::from(b3) << 24)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::io::Cursor;

    use camino::Utf8Path;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::testing::{CheckpointMode, SyntheticWal, WalFixture};

    /// Small page size keeps synthetic WALs compact.
    const PAGE: u32 = 512;
    const SALT: (u32, u32) = (0x1122_3344, 0x5566_7788);

    fn page_of(fill: u8) -> Vec<u8> {
        vec![fill; usize::try_from(PAGE).unwrap()]
    }

    fn push_be(bytes: &mut Vec<u8>, value: u32) {
        bytes.push(u8::try_from(value >> 24).unwrap());
        bytes.push(u8::try_from((value >> 16) & 0xff).unwrap());
        bytes.push(u8::try_from((value >> 8) & 0xff).unwrap());
        bytes.push(u8::try_from(value & 0xff).unwrap());
    }

    /// Hand-build a 32-byte WAL header with a correct self-checksum.
    fn build_header(
        magic: u32,
        format: u32,
        page_size: u32,
        seq: u32,
        salt: (u32, u32),
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(32);
        push_be(&mut header, magic);
        push_be(&mut header, format);
        push_be(&mut header, page_size);
        push_be(&mut header, seq);
        push_be(&mut header, salt.0);
        push_be(&mut header, salt.1);
        let checksum = wal_checksum(magic == WAL_MAGIC_BE, (0, 0), &header);
        push_be(&mut header, checksum.0);
        push_be(&mut header, checksum.1);
        header
    }

    fn scan_all(bytes: &[u8]) -> (WalHeader, Vec<CommittedChunk>) {
        let mut scanner = WalScanner::open(Cursor::new(bytes.to_vec()))
            .unwrap()
            .unwrap();
        let header = *scanner.header();
        let mut chunks = Vec::new();
        while let Some(chunk) = scanner.next_committed(u64::MAX).unwrap() {
            chunks.push(chunk);
        }
        (header, chunks)
    }

    /// Total shippable end offset over a whole WAL byte string.
    fn shippable_end(bytes: &[u8]) -> u64 {
        let (_, chunks) = scan_all(bytes);
        chunks
            .last()
            .map_or(WAL_HEADER_SIZE, |chunk| chunk.end_offset)
    }

    fn chunk_frames(header: &WalHeader, chunk: &CommittedChunk) -> Vec<FrameHeader> {
        let frame_size = usize::try_from(header.frame_size()).unwrap();
        chunk
            .bytes
            .chunks_exact(frame_size)
            .map(|frame| {
                let mut head = [0u8; 24];
                head.copy_from_slice(&frame[..24]);
                FrameHeader::parse(&head)
            })
            .collect()
    }

    fn tempdir_path(dir: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(dir.path()).unwrap()
    }

    // 1. Header parsing and the checksum fold

    #[test]
    fn checksum_fold_known_vectors() {
        // One pair, little-endian words: x = (1, 2)
        let data = [1, 0, 0, 0, 2, 0, 0, 0];
        assert_eq!(wal_checksum(false, (0, 0), &data), (1, 3));
        // Same bytes as big-endian words
        assert_eq!(
            wal_checksum(true, (0, 0), &data),
            (0x0100_0000, 0x0300_0000)
        );
        // Two pairs, big-endian words: x = (1, 2, 3, 4)
        // s1 = 1, s2 = 3; then s1 = 1 + 3 + 3 = 7, s2 = 3 + 4 + 7 = 14
        let data = [0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4];
        assert_eq!(wal_checksum(true, (0, 0), &data), (7, 14));
        // Seeding continues the chain
        assert_eq!(wal_checksum(true, (1, 3), &data[8..]), (7, 14));
        // Empty input returns the seed
        assert_eq!(wal_checksum(false, (9, 9), &[]), (9, 9));
    }

    #[test]
    fn reject_truncated_header() {
        for len in [0usize, 1, 31] {
            let bytes = vec![0u8; len];
            let err = parse_wal_header(&bytes).unwrap_err();
            assert!(
                matches!(err, WalError::TruncatedHeader(n) if n == len),
                "expected TruncatedHeader({len}), got {err:?}"
            );
        }
    }

    #[test]
    fn reject_bad_magic() {
        let header = build_header(0xdead_beef, WAL_FORMAT, PAGE, 0, SALT);
        let err = parse_wal_header(&header).unwrap_err();
        assert!(
            matches!(err, WalError::BadMagic(0xdead_beef)),
            "expected BadMagic, got {err:?}"
        );
        // WalScanner::open propagates header errors (32 bytes is not "short")
        let Err(err) = WalScanner::open(Cursor::new(header)) else {
            panic!("expected BadMagic from open")
        };
        assert!(
            matches!(err, WalError::BadMagic(0xdead_beef)),
            "expected BadMagic from open, got {err:?}"
        );
    }

    #[test]
    fn reject_unsupported_format() {
        let header = build_header(WAL_MAGIC_LE, 3_007_001, PAGE, 0, SALT);
        let err = parse_wal_header(&header).unwrap_err();
        assert!(
            matches!(err, WalError::UnsupportedFormat(3_007_001)),
            "expected UnsupportedFormat, got {err:?}"
        );
    }

    #[test]
    fn reject_invalid_page_size() {
        for page_size in [0u32, 1, 100, 256, 1024 + 1, 2 * MAX_PAGE_SIZE] {
            let header = build_header(WAL_MAGIC_LE, WAL_FORMAT, page_size, 0, SALT);
            let err = parse_wal_header(&header).unwrap_err();
            assert!(
                matches!(err, WalError::InvalidPageSize(n) if n == page_size),
                "expected InvalidPageSize({page_size}), got {err:?}"
            );
        }
    }

    #[test]
    fn reject_header_checksum_mismatch() {
        // Corrupt a covered byte (a salt byte, so magic/format/page_size
        // still parse) after the checksum was computed
        let mut header = build_header(WAL_MAGIC_LE, WAL_FORMAT, PAGE, 0, SALT);
        header[17] ^= 0x01;
        let err = parse_wal_header(&header).unwrap_err();
        assert!(
            matches!(err, WalError::HeaderChecksum { stored, computed } if stored != computed),
            "expected HeaderChecksum, got {err:?}"
        );
        // Corrupt a stored checksum byte (24..32)
        let mut header = build_header(WAL_MAGIC_LE, WAL_FORMAT, PAGE, 0, SALT);
        header[25] ^= 0x80;
        let err = parse_wal_header(&header).unwrap_err();
        assert!(
            matches!(err, WalError::HeaderChecksum { .. }),
            "expected HeaderChecksum, got {err:?}"
        );
    }

    // 2. Empty and header-only WALs

    #[test]
    fn parse_empty_file() {
        for len in [0usize, 1, 31] {
            let scanner = WalScanner::open(Cursor::new(vec![0u8; len])).unwrap();
            assert!(
                scanner.is_none(),
                "expected Ok(None) for a {len}-byte WAL file"
            );
        }
    }

    #[test]
    fn parse_header_only_wal() {
        let bytes = SyntheticWal::new(PAGE, false, SALT).bytes();
        assert_eq!(bytes.len(), 32);
        let mut scanner = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(scanner.header().page_size, PAGE);
        assert_eq!(scanner.header().salt, SALT);
        assert_eq!(scanner.offset(), WAL_HEADER_SIZE);
        assert!(
            scanner.next_committed(u64::MAX).unwrap().is_none(),
            "a header-only WAL has nothing to ship"
        );
        assert_eq!(scanner.offset(), WAL_HEADER_SIZE);
    }

    // 3. Real-WAL fixtures: header and frame iteration

    #[test]
    fn parse_valid_header_all_page_sizes() {
        for page_size in [512u32, 4096, 8192, MAX_PAGE_SIZE] {
            let tmp = tempfile::tempdir().unwrap();
            let fixture = WalFixture::new(tempdir_path(&tmp), page_size).unwrap();
            let wal = fixture.wal_bytes().unwrap();
            assert!(
                wal.len() > 32,
                "fixture WAL for page size {page_size} should have frames"
            );
            let header = parse_wal_header(&wal).unwrap();
            assert_eq!(header.page_size, page_size, "page size {page_size}");
            assert_eq!(header.format, WAL_FORMAT, "page size {page_size}");
            assert!(
                header.magic == WAL_MAGIC_LE || header.magic == WAL_MAGIC_BE,
                "unexpected magic {:#010x}",
                header.magic
            );
            // Everything the fixture wrote is committed: the shippable
            // prefix covers the whole file.
            let (_, chunks) = scan_all(&wal);
            let end = chunks.last().map(|chunk| chunk.end_offset);
            assert_eq!(
                end,
                Some(u64::try_from(wal.len()).unwrap()),
                "page size {page_size}"
            );
        }
    }

    #[test]
    fn parse_single_commit_frame() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        // Reset the WAL so it contains exactly one transaction
        fixture.checkpoint(CheckpointMode::Truncate).unwrap();
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('one')"])
            .unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let (header, chunks) = scan_all(&wal);
        assert_eq!(chunks.len(), 1, "one committed chunk");
        let chunk = &chunks[0];
        assert_eq!(chunk.commit_count, 1, "one commit frame");
        assert_eq!(chunk.start_offset, WAL_HEADER_SIZE);
        assert_eq!(chunk.end_offset, u64::try_from(wal.len()).unwrap());
        let frames = chunk_frames(&header, chunk);
        assert!(!frames.is_empty(), "at least one frame");
        let last = frames.last().unwrap();
        assert!(last.is_commit(), "last frame carries the commit flag");
        assert_eq!(chunk.db_size_pages, last.db_size);
        for frame in &frames[..frames.len() - 1] {
            assert!(!frame.is_commit(), "only the last frame commits");
        }
    }

    #[test]
    fn parse_multi_frame_single_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        fixture.checkpoint(CheckpointMode::Truncate).unwrap();
        fixture.txn_touching_pages(8).unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let (header, chunks) = scan_all(&wal);
        assert_eq!(chunks.len(), 1, "one committed chunk");
        let chunk = &chunks[0];
        assert_eq!(chunk.commit_count, 1, "a single commit");
        let frames = chunk_frames(&header, chunk);
        assert!(
            frames.len() > 1,
            "expected a multi-frame transaction, got {} frame(s)",
            frames.len()
        );
        assert_eq!(chunk.end_offset, u64::try_from(wal.len()).unwrap());
    }

    // 4. The crate's most important test: uncommitted frames never ship

    #[test]
    fn uncommitted_tail_frames_not_shipped() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        fixture.checkpoint(CheckpointMode::Truncate).unwrap();
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('committed')"])
            .unwrap();
        let committed_len = u64::try_from(fixture.wal_bytes().unwrap().len()).unwrap();

        // Spill a big transaction into the WAL, then roll it back: the WAL
        // now ends with flushed-but-uncommitted frames.
        fixture.big_txn_spilling(false).unwrap();
        let wal = fixture.wal_bytes().unwrap();
        assert!(
            u64::try_from(wal.len()).unwrap() > committed_len,
            "rolled-back transaction must have spilled frames into the WAL"
        );

        // The shippable prefix ends exactly at the last commit; not one byte
        // of the rolled-back tail is surfaced.
        assert_eq!(shippable_end(&wal), committed_len);

        // Small max_bytes chunking never crosses the boundary either
        let mut scanner = WalScanner::open(Cursor::new(wal.clone())).unwrap().unwrap();
        while let Some(chunk) = scanner.next_committed(1).unwrap() {
            assert!(
                chunk.end_offset <= committed_len,
                "chunk end {} crosses the last commit at {committed_len}",
                chunk.end_offset
            );
        }
        assert_eq!(scanner.offset(), committed_len);

        // Once a spilling transaction commits, its frames become shippable
        fixture.big_txn_spilling(true).unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let end = shippable_end(&wal);
        assert!(
            end > committed_len,
            "committed spill must ship: end {end} <= {committed_len}"
        );
        assert!(end <= u64::try_from(wal.len()).unwrap(), "end within file");
    }

    // 5. Chain breaks: corruption ends the trusted prefix

    #[test]
    fn chain_break_bit_flip_in_frame_header() {
        let frame_size = usize::try_from(FRAME_HEADER_SIZE + u64::from(PAGE)).unwrap();
        // Flip a page_no bit in the second frame's header
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .flip_bit(32 + frame_size + 3, 0)
            .bytes();
        let (_, chunks) = scan_all(&bytes);
        assert_eq!(chunks.len(), 1, "only the frame before the break ships");
        assert_eq!(
            chunks[0].end_offset,
            32 + u64::try_from(frame_size).unwrap()
        );
        assert_eq!(chunks[0].commit_count, 1, "one commit before the break");
    }

    #[test]
    fn chain_break_bit_flip_in_page_data() {
        let frame_size = usize::try_from(FRAME_HEADER_SIZE + u64::from(PAGE)).unwrap();
        // Flip a payload bit in the second frame
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .flip_bit(32 + frame_size + 24 + 100, 7)
            .bytes();
        let (_, chunks) = scan_all(&bytes);
        assert_eq!(chunks.len(), 1, "only the frame before the break ships");
        assert_eq!(
            chunks[0].end_offset,
            32 + u64::try_from(frame_size).unwrap()
        );
    }

    // 6. Checksum byte orders

    #[test]
    fn checksum_verified_both_byte_orders() {
        let mut ends = Vec::new();
        for big_endian in [false, true] {
            let bytes = SyntheticWal::new(PAGE, big_endian, SALT)
                .frame(1, &page_of(0x11))
                .commit_frame(2, &page_of(0x22), 2)
                .commit_frame(1, &page_of(0x33), 2)
                .bytes();
            let (header, chunks) = scan_all(&bytes);
            assert_eq!(
                header.big_endian_checksum(),
                big_endian,
                "magic selects the checksum word order"
            );
            let end = chunks.last().unwrap().end_offset;
            assert_eq!(
                end,
                u64::try_from(bytes.len()).unwrap(),
                "all frames verify in {} order",
                if big_endian {
                    "big-endian"
                } else {
                    "little-endian"
                }
            );
            let commits: u64 = chunks.iter().map(|chunk| chunk.commit_count).sum();
            assert_eq!(commits, 2, "both commits found");
            ends.push(end);
        }
        assert_eq!(ends[0], ends[1], "same logical content, same end offset");
    }

    // 7. Stale salts (leftovers from before a WAL restart)

    #[test]
    fn reject_stale_salt_frames() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        let stale_salt = (SALT.0 ^ 1, SALT.1);
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .frame_with_salts(2, &page_of(0xbb), stale_salt)
            .commit_frame(3, &page_of(0xcc), 3)
            .bytes();
        let (_, chunks) = scan_all(&bytes);
        assert_eq!(chunks.len(), 1, "the valid prefix ends at the stale salt");
        assert_eq!(chunks[0].end_offset, 32 + frame_size);
        assert_eq!(chunks[0].commit_count, 1, "one commit before the break");
    }

    // 8. Torn tails

    #[test]
    fn frame_exactly_at_eof() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        // A complete but uncommitted frame at EOF is not shipped
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .frame(2, &page_of(0xbb))
            .bytes();
        assert_eq!(shippable_end(&bytes), 32 + frame_size);
        // A commit frame ending exactly at EOF is shipped in full
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .bytes();
        assert_eq!(shippable_end(&bytes), 32 + 2 * frame_size);
    }

    #[test]
    fn mid_frame_eof() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        let torn_at = usize::try_from(32 + frame_size + 300).unwrap();
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .truncate_at(torn_at)
            .bytes();
        assert_eq!(
            shippable_end(&bytes),
            32 + frame_size,
            "the torn frame is not shipped"
        );
        // Torn inside the 24-byte frame header
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .truncate_at(usize::try_from(32 + frame_size + 10).unwrap())
            .bytes();
        assert_eq!(shippable_end(&bytes), 32 + frame_size);
    }

    // 9. Commit metadata and chunking

    #[test]
    fn commit_frame_db_size_tracked() {
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 3)
            .commit_frame(2, &page_of(0xbb), 7)
            .bytes();
        // One big chunk reports the last commit's db_size
        let (_, chunks) = scan_all(&bytes);
        assert_eq!(chunks.len(), 1, "single chunk");
        assert_eq!(chunks[0].commit_count, 2, "two commits");
        assert_eq!(chunks[0].db_size_pages, 7, "last commit wins");
        // Per-commit chunks report each commit's db_size
        let mut scanner = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        let first = scanner.next_committed(1).unwrap().unwrap();
        assert_eq!(first.db_size_pages, 3);
        let second = scanner.next_committed(1).unwrap().unwrap();
        assert_eq!(second.db_size_pages, 7);
    }

    #[test]
    fn max_bytes_chunking_ends_on_commit_boundaries() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .bytes();
        let mut scanner = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        let mut previous_end = WAL_HEADER_SIZE;
        let mut previous_checksum = scanner.checksum();
        for index in 0..3u64 {
            let chunk = scanner.next_committed(1).unwrap().unwrap();
            assert_eq!(chunk.start_offset, previous_end, "chunks are contiguous");
            assert_eq!(
                chunk.end_offset,
                32 + (index + 1) * frame_size,
                "each chunk ends on the next commit boundary"
            );
            assert_eq!(chunk.commit_count, 1, "one commit per chunk");
            assert_eq!(
                chunk.bytes.len(),
                usize::try_from(frame_size).unwrap(),
                "chunk bytes cover exactly the returned frames"
            );
            assert_ne!(
                chunk.checksum_at_end, previous_checksum,
                "the running checksum advances"
            );
            previous_end = chunk.end_offset;
            previous_checksum = chunk.checksum_at_end;
        }
        assert!(scanner.next_committed(1).unwrap().is_none(), "clean EOF");
        assert!(
            scanner.next_committed(1).unwrap().is_none(),
            "EOF is idempotent"
        );
    }

    #[test]
    fn oversized_transaction_returned_whole() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        // Four non-commit frames then the commit: one transaction
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .frame(1, &page_of(0x01))
            .frame(2, &page_of(0x02))
            .frame(3, &page_of(0x03))
            .frame(4, &page_of(0x04))
            .commit_frame(5, &page_of(0x05), 5)
            .bytes();
        let mut scanner = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        // max_bytes far below the transaction size
        let chunk = scanner.next_committed(100).unwrap().unwrap();
        assert_eq!(chunk.commit_count, 1, "the whole transaction is returned");
        assert_eq!(chunk.end_offset, 32 + 5 * frame_size);
        assert!(
            chunk.bytes.len() > 100,
            "a single transaction may exceed max_bytes"
        );
    }

    // 10. Resume

    #[test]
    fn resume_continues_chain() {
        // Synthetic: chunk, then resume from the recorded position
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .bytes();
        let (header, continuous) = scan_all(&bytes);
        let continuous_bytes: Vec<u8> = continuous
            .iter()
            .flat_map(|chunk| chunk.bytes.clone())
            .collect();

        let mut scanner = WalScanner::open(Cursor::new(bytes.clone()))
            .unwrap()
            .unwrap();
        let first = scanner.next_committed(1).unwrap().unwrap();
        drop(scanner);
        let mut resumed = WalScanner::resume(
            Cursor::new(bytes),
            header,
            first.end_offset,
            first.checksum_at_end,
        )
        .unwrap();
        let mut resumed_bytes = first.bytes.clone();
        while let Some(chunk) = resumed.next_committed(u64::MAX).unwrap() {
            resumed_bytes.extend_from_slice(&chunk.bytes);
        }
        assert_eq!(
            resumed_bytes, continuous_bytes,
            "resume yields the same bytes as one continuous scan"
        );

        // Real WAL: scan, append more commits, resume from the old position
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let (header, chunks) = scan_all(&wal);
        let position = chunks.last().unwrap();
        let (offset, checksum) = (position.end_offset, position.checksum_at_end);
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('after-resume')"])
            .unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let mut resumed =
            WalScanner::resume(Cursor::new(wal.clone()), header, offset, checksum).unwrap();
        let tail = resumed.next_committed(u64::MAX).unwrap().unwrap();
        assert_eq!(tail.start_offset, offset);
        assert_eq!(tail.end_offset, u64::try_from(wal.len()).unwrap());
        // Identical to the tail of one continuous scan
        let (_, full) = scan_all(&wal);
        let full_bytes: Vec<u8> = full.iter().flat_map(|chunk| chunk.bytes.clone()).collect();
        let split = usize::try_from(offset - WAL_HEADER_SIZE).unwrap();
        assert_eq!(&full_bytes[split..], &tail.bytes[..]);
    }

    #[test]
    fn resume_rejects_misaligned_offset() {
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .bytes();
        let header = parse_wal_header(&bytes).unwrap();
        let frame_size = header.frame_size();
        for offset in [0, 31, 33, 32 + frame_size - 1, 32 + frame_size + 1] {
            let Err(err) = WalScanner::resume(Cursor::new(bytes.clone()), header, offset, (0, 0))
            else {
                panic!("expected MisalignedOffset for offset {offset}")
            };
            assert!(
                matches!(err, WalError::MisalignedOffset { offset: o, page_size } if o == offset && page_size == PAGE),
                "expected MisalignedOffset for {offset}, got {err:?}"
            );
        }
        // Frame-aligned offsets are accepted
        for offset in [32, 32 + frame_size] {
            WalScanner::resume(Cursor::new(bytes.clone()), header, offset, (0, 0)).unwrap();
        }
    }

    // 10b. pgno-0 frames end the prefix (walDecodeFrame rejects pgno 0)

    #[test]
    fn pgno_zero_frame_stops_prefix() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        // A checksum-valid frame carrying page number 0 sits between two
        // otherwise-valid commits. The scan must stop before it.
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(0, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .bytes();
        let (_, chunks) = scan_all(&bytes);
        assert_eq!(chunks.len(), 1, "the valid prefix ends at the pgno-0 frame");
        assert_eq!(chunks[0].end_offset, 32 + frame_size);
        assert_eq!(
            chunks[0].commit_count, 1,
            "one commit before the pgno-0 frame"
        );
    }

    // 10c. Scan-only (discard) mode

    #[test]
    fn scan_committed_extent_agrees_with_next_committed() {
        // A multi-transaction WAL, some multi-frame. The discard scan must
        // land on the exact offset and checksum the retaining scan reaches.
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .frame(2, &page_of(0xbb))
            .commit_frame(3, &page_of(0xcc), 3)
            .commit_frame(4, &page_of(0xdd), 4)
            .bytes();
        // Retaining scan: walk to the end, recording the final offset/checksum.
        let mut retaining = WalScanner::open(Cursor::new(bytes.clone()))
            .unwrap()
            .unwrap();
        let mut retained_end = WAL_HEADER_SIZE;
        let mut retained_checksum = retaining.checksum();
        while let Some(chunk) = retaining.next_committed(u64::MAX).unwrap() {
            retained_end = chunk.end_offset;
            retained_checksum = chunk.checksum_at_end;
        }
        // Discard scan: one pass with no size cap covers the whole WAL.
        let mut discarding = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        let extent = discarding
            .scan_committed_extent(u64::MAX, u64::MAX)
            .unwrap();
        assert_eq!(extent, retained_end, "discard scan reaches the same offset");
        assert_eq!(discarding.offset(), retained_end);
        assert_eq!(
            discarding.checksum(),
            retained_checksum,
            "discard scan recovers the same running checksum"
        );
    }

    #[test]
    fn scan_committed_extent_bounds_oversized_open_transaction() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        // One commit, then a long OPEN (never committed) run of valid frames.
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0x01), 1)
            .frame(2, &page_of(0x02))
            .frame(3, &page_of(0x03))
            .frame(4, &page_of(0x04))
            .frame(5, &page_of(0x05))
            .bytes();
        // A cap just above a single frame aborts once the open run since the
        // last commit exceeds it, instead of scanning the whole tail.
        let cap = frame_size + 1;
        let mut scanner = WalScanner::open(Cursor::new(bytes)).unwrap().unwrap();
        let err = scanner.scan_committed_extent(u64::MAX, cap).unwrap_err();
        assert!(
            matches!(err, WalError::TransactionTooLarge { max_bytes, .. } if max_bytes == cap),
            "expected TransactionTooLarge, got {err:?}"
        );
    }

    // 10d. Read errors mid-scan propagate (never a silent short extent)

    /// A `Read + Seek` that serves `fail_after` bytes total, then errors on
    /// every subsequent read (a disk fault partway through the WAL).
    struct ErroringReader {
        inner: Cursor<Vec<u8>>,
        fail_after: u64,
        read_total: u64,
    }

    impl Read for ErroringReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.read_total >= self.fail_after {
                return Err(std::io::Error::other("injected mid-WAL read error"));
            }
            let budget = usize::try_from(self.fail_after - self.read_total).unwrap_or(usize::MAX);
            let cap = buf.len().min(budget);
            let read = self.inner.read(&mut buf[..cap])?;
            self.read_total += u64::try_from(read).unwrap_or(0);
            Ok(read)
        }
    }

    impl Seek for ErroringReader {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn scan_propagates_mid_wal_read_error() {
        let frame_size = FRAME_HEADER_SIZE + u64::from(PAGE);
        let bytes = SyntheticWal::new(PAGE, false, SALT)
            .commit_frame(1, &page_of(0xaa), 1)
            .commit_frame(2, &page_of(0xbb), 2)
            .commit_frame(3, &page_of(0xcc), 3)
            .bytes();
        // Serve the header plus the first frame, then error partway through
        // the second frame: the first commit is valid but the scan must NOT
        // return that short extent, it must surface the read error.
        let fail_after = WAL_HEADER_SIZE + frame_size + 5;
        let make_scanner = || {
            WalScanner::open(ErroringReader {
                inner: Cursor::new(bytes.clone()),
                fail_after,
                read_total: 0,
            })
            .unwrap()
            .unwrap()
        };
        let err = make_scanner().next_committed(u64::MAX).unwrap_err();
        assert!(
            matches!(err, WalError::Read(_)),
            "next_committed must propagate the read error, got {err:?}"
        );
        let err = make_scanner()
            .scan_committed_extent(u64::MAX, u64::MAX)
            .unwrap_err();
        assert!(
            matches!(err, WalError::Read(_)),
            "scan_committed_extent must propagate the read error, got {err:?}"
        );
    }

    // 11. Cross-validation against SQLite itself

    #[test]
    fn parser_agrees_with_sqlite_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        fixture
            .txn(&[
                "INSERT INTO t (data) VALUES ('alpha')",
                "INSERT INTO t (data) VALUES ('beta')",
            ])
            .unwrap();
        fixture.txn_touching_pages(8).unwrap();
        fixture.txn(&["DELETE FROM t WHERE data = 'beta'"]).unwrap();
        // Leave a rolled-back tail in the WAL
        fixture.big_txn_spilling(false).unwrap();
        let db = fixture.db_bytes().unwrap();
        let wal = fixture.wal_bytes().unwrap();

        // SQLite's own recovery: copy db + wal, checkpoint, read
        let sqlite_dir = tempfile::tempdir().unwrap();
        let sqlite_db = sqlite_dir.path().join("recovered.db");
        std::fs::write(&sqlite_db, &db).unwrap();
        std::fs::write(sqlite_dir.path().join("recovered.db-wal"), &wal).unwrap();
        let conn = rusqlite::Connection::open(&sqlite_db).unwrap();
        let _row: i64 = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
            .unwrap();
        let sqlite_rows = read_rows(&conn);
        drop(conn);

        // Our parser: apply each committed frame's page to the db image
        let mut scanner = WalScanner::open(Cursor::new(wal)).unwrap().unwrap();
        let header = *scanner.header();
        let page_size = usize::try_from(header.page_size).unwrap();
        let frame_size = usize::try_from(header.frame_size()).unwrap();
        let mut image = db;
        let mut db_pages = 0u32;
        while let Some(chunk) = scanner.next_committed(u64::MAX).unwrap() {
            for frame in chunk.bytes.chunks_exact(frame_size) {
                let mut head = [0u8; 24];
                head.copy_from_slice(&frame[..24]);
                let frame_header = FrameHeader::parse(&head);
                // pgno 0 is rejected by the scanner, so page_no >= 1 here;
                // checked_sub keeps the helper safe if that ever regresses.
                let page_index = frame_header.page_no.checked_sub(1).expect("page_no >= 1");
                let at = usize::try_from(page_index).unwrap() * page_size;
                if image.len() < at + page_size {
                    image.resize(at + page_size, 0);
                }
                image[at..at + page_size].copy_from_slice(&frame[24..]);
            }
            db_pages = chunk.db_size_pages;
        }
        assert!(db_pages > 0, "at least one commit applied");
        image.truncate(usize::try_from(db_pages).unwrap() * page_size);

        let parsed_dir = tempfile::tempdir().unwrap();
        let parsed_db = parsed_dir.path().join("parsed.db");
        std::fs::write(&parsed_db, &image).unwrap();
        let conn = rusqlite::Connection::open(&parsed_db).unwrap();
        let check: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(check, "ok", "recovered image passes integrity_check");
        let parsed_rows = read_rows(&conn);
        assert_eq!(
            parsed_rows, sqlite_rows,
            "applying parsed committed frames reproduces SQLite's own recovery"
        );
    }

    #[test]
    fn synthetic_wal_accepted_by_sqlite() {
        // Build a real WAL, then rebuild it byte-for-byte with SyntheticWal
        let tmp = tempfile::tempdir().unwrap();
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('alpha')"])
            .unwrap();
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('beta')"])
            .unwrap();
        let db = fixture.db_bytes().unwrap();
        let wal = fixture.wal_bytes().unwrap();

        let header = parse_wal_header(&wal).unwrap();
        assert_eq!(
            header.checkpoint_seq, 0,
            "a fresh database's first WAL starts at checkpoint_seq 0"
        );
        let page_size = usize::try_from(header.page_size).unwrap();
        let frame_size = usize::try_from(header.frame_size()).unwrap();
        assert!(
            (wal.len() - 32).is_multiple_of(frame_size),
            "fixture WAL is frame-aligned"
        );

        let mut synthetic =
            SyntheticWal::new(header.page_size, header.big_endian_checksum(), header.salt);
        for frame in wal[32..].chunks_exact(frame_size) {
            let mut head = [0u8; 24];
            head.copy_from_slice(&frame[..24]);
            let frame_header = FrameHeader::parse(&head);
            let page = &frame[24..24 + page_size];
            synthetic = if frame_header.is_commit() {
                synthetic.commit_frame(frame_header.page_no, page, frame_header.db_size)
            } else {
                synthetic.frame(frame_header.page_no, page)
            };
        }
        let rebuilt = synthetic.bytes();
        assert_eq!(rebuilt.len(), wal.len(), "rebuilt WAL has the same length");
        assert!(
            rebuilt == wal,
            "SyntheticWal reproduces the real WAL byte-for-byte (checksum chain matches SQLite)"
        );

        // SQLite recovers the synthetic WAL next to the db file image
        let recover_dir = tempfile::tempdir().unwrap();
        let recover_db = recover_dir.path().join("synthetic.db");
        std::fs::write(&recover_db, &db).unwrap();
        std::fs::write(recover_dir.path().join("synthetic.db-wal"), &rebuilt).unwrap();
        let conn = rusqlite::Connection::open(&recover_db).unwrap();
        let rows = read_rows(&conn);
        let data: Vec<&str> = rows.iter().map(|(_, data)| data.as_str()).collect();
        assert_eq!(
            data,
            vec!["init", "alpha", "beta"],
            "SQLite recovers the expected rows from the synthetic WAL"
        );
    }

    fn read_rows(conn: &rusqlite::Connection) -> Vec<(i64, String)> {
        let mut stmt = conn.prepare("SELECT id, data FROM t ORDER BY id").unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    // 12. Golden files: a committed real {db, wal, dump} triple

    /// Rows the golden fixture contains ('init' comes from `WalFixture::new`).
    const GOLDEN_ROWS: &[(i64, &str)] = &[(1, "init"), (2, "alpha"), (3, "beta"), (4, "gamma")];

    /// Render a human-readable, stable dump of every committed frame:
    /// offset, `page_no`, `db_size`, and commit flag, plus header fields and
    /// end-of-scan totals.
    fn render_dump(wal: &[u8]) -> String {
        let mut scanner = WalScanner::open(Cursor::new(wal.to_vec()))
            .unwrap()
            .unwrap();
        let header = *scanner.header();
        let mut out = String::new();
        writeln!(
            out,
            "magic {}",
            if header.big_endian_checksum() {
                "big_endian"
            } else {
                "little_endian"
            }
        )
        .unwrap();
        writeln!(out, "page_size {}", header.page_size).unwrap();
        writeln!(out, "checkpoint_seq {}", header.checkpoint_seq).unwrap();
        writeln!(out, "salt {:#010x} {:#010x}", header.salt.0, header.salt.1).unwrap();
        let frame_size = usize::try_from(header.frame_size()).unwrap();
        let mut offset = WAL_HEADER_SIZE;
        let mut end_offset = WAL_HEADER_SIZE;
        let mut commit_count = 0u64;
        let mut db_size_pages = 0u32;
        while let Some(chunk) = scanner.next_committed(u64::MAX).unwrap() {
            for frame in chunk.bytes.chunks_exact(frame_size) {
                let mut head = [0u8; 24];
                head.copy_from_slice(&frame[..24]);
                let frame_header = FrameHeader::parse(&head);
                writeln!(
                    out,
                    "frame offset={offset} page_no={} db_size={} commit={}",
                    frame_header.page_no,
                    frame_header.db_size,
                    frame_header.is_commit()
                )
                .unwrap();
                offset += header.frame_size();
            }
            end_offset = chunk.end_offset;
            commit_count += chunk.commit_count;
            db_size_pages = chunk.db_size_pages;
        }
        writeln!(out, "end_offset {end_offset}").unwrap();
        writeln!(out, "commit_count {commit_count}").unwrap();
        writeln!(out, "db_size_pages {db_size_pages}").unwrap();
        out
    }

    #[test]
    fn golden_wal_parses_to_expected_dump() {
        let wal: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/golden/wal_le_4096.wal"
        ));
        let db: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/golden/wal_le_4096.db"
        ));
        let dump = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/golden/wal_le_4096.dump.txt"
        ));

        // The parser reproduces the committed dump exactly
        assert_eq!(render_dump(wal), dump, "golden dump drifted");
        let header = parse_wal_header(wal).unwrap();
        assert_eq!(header.magic, WAL_MAGIC_LE, "golden WAL is little-endian");
        assert_eq!(header.page_size, 4096);

        // Cross-SQLite-version canary: whatever SQLite is linked today must
        // recover the same rows from the committed byte images
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("golden.db");
        std::fs::write(&db_path, db).unwrap();
        std::fs::write(tmp.path().join("golden.db-wal"), wal).unwrap();
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let rows = read_rows(&conn);
        let expected: Vec<(i64, String)> = GOLDEN_ROWS
            .iter()
            .map(|&(id, data)| (id, data.to_owned()))
            .collect();
        assert_eq!(rows, expected, "SQLite recovers the golden rows");
    }

    /// Regenerate the committed golden files. Run manually during
    /// development when the golden triple must change:
    ///
    /// ```text
    /// cargo nextest run -p bencher_replica --features plus,testing \
    ///     --run-ignored ignored-only -E 'test(generate_golden_files)'
    /// ```
    #[test]
    #[ignore = "regenerates the committed golden files under golden/"]
    fn generate_golden_files() {
        let golden_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("golden");
        std::fs::create_dir_all(&golden_dir).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        // WalFixture::new commits the schema and the 'init' row
        let fixture = WalFixture::new(tempdir_path(&tmp), 4096).unwrap();
        fixture
            .txn(&[
                "INSERT INTO t (data) VALUES ('alpha')",
                "INSERT INTO t (data) VALUES ('beta')",
            ])
            .unwrap();
        fixture
            .txn(&["INSERT INTO t (data) VALUES ('gamma')"])
            .unwrap();
        let db = fixture.db_bytes().unwrap();
        let wal = fixture.wal_bytes().unwrap();
        let header = parse_wal_header(&wal).unwrap();
        assert_eq!(
            header.magic, WAL_MAGIC_LE,
            "golden files must be generated on a little-endian host"
        );
        std::fs::write(golden_dir.join("wal_le_4096.db"), &db).unwrap();
        std::fs::write(golden_dir.join("wal_le_4096.wal"), &wal).unwrap();
        std::fs::write(golden_dir.join("wal_le_4096.dump.txt"), render_dump(&wal)).unwrap();
    }
}
