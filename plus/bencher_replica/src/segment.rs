//! WAL segment compression.
//!
//! A segment is a run of raw WAL bytes `[start, end)` that always ends on a
//! commit-frame boundary (commit alignment is enforced upstream by
//! [`crate::wal::WalScanner::next_committed`]). Segments are stored
//! zstd-compressed with zstd's built-in content checksum enabled, so a
//! corrupted object fails loudly at decode time rather than yielding garbage
//! frames.

use std::io::{Read as _, Write as _};

/// Maximum raw (uncompressed) WAL bytes per segment. A single transaction
/// larger than this is shipped as one oversized segment: commit atomicity
/// beats the cap.
pub const SEGMENT_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// zstd compression level for segments and snapshots.
pub const ZSTD_LEVEL: i32 = 3;

/// Hard cap on decompressed segment output. Segments are bounded well below
/// this (see [`SEGMENT_MAX_BYTES`]; a single oversized transaction is the only
/// exception), so anything larger is a corrupt or hostile object
/// (zstd-bomb), not data: refuse instead of allocating without bound.
pub const MAX_DECOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SegmentError {
    #[error("Failed to compress segment: {0}")]
    Compress(std::io::Error),
    #[error("Failed to decompress segment: {0}")]
    Decompress(std::io::Error),
    #[error("Decompressed segment exceeds the {max_bytes} byte cap")]
    TooLarge { max_bytes: u64 },
}

/// Compress raw WAL bytes (zstd, content checksum on).
pub fn compress_segment(raw: &[u8]) -> Result<Vec<u8>, SegmentError> {
    let mut encoder =
        zstd::stream::Encoder::new(Vec::new(), ZSTD_LEVEL).map_err(SegmentError::Compress)?;
    encoder
        .include_checksum(true)
        .map_err(SegmentError::Compress)?;
    encoder.write_all(raw).map_err(SegmentError::Compress)?;
    encoder.finish().map_err(SegmentError::Compress)
}

/// Decompress a segment object back to raw WAL bytes, verifying the zstd
/// content checksum and refusing output beyond [`MAX_DECOMPRESSED_BYTES`].
pub fn decompress_segment(compressed: &[u8]) -> Result<Vec<u8>, SegmentError> {
    decompress_segment_with_cap(compressed, MAX_DECOMPRESSED_BYTES)
}

/// Decompress with an explicit output cap; reading stops one byte past the
/// cap, so a zstd bomb never allocates more than `cap + 1` bytes. Restore
/// passes the segment's exact expected size so an oversized or hostile
/// object is rejected up front rather than after a multi-gigabyte allocation.
pub(crate) fn decompress_segment_with_cap(
    compressed: &[u8],
    cap: u64,
) -> Result<Vec<u8>, SegmentError> {
    let decoder =
        zstd::stream::Decoder::with_buffer(compressed).map_err(SegmentError::Decompress)?;
    let mut raw = Vec::new();
    decoder
        .take(cap.saturating_add(1))
        .read_to_end(&mut raw)
        .map_err(SegmentError::Decompress)?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > cap {
        return Err(SegmentError::TooLarge { max_bytes: cap });
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        MAX_DECOMPRESSED_BYTES, SegmentError, compress_segment, decompress_segment,
        decompress_segment_with_cap,
    };

    /// Deterministic pseudo-random bytes (xorshift64): incompressible enough
    /// that the corruption tests have a real compressed body to flip bits in.
    fn noise_bytes(len: usize) -> Vec<u8> {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push(u8::try_from(state >> 56).unwrap());
        }
        bytes
    }

    #[test]
    fn zstd_roundtrip_identity() {
        let raw = noise_bytes(64 * 1024);
        let compressed = compress_segment(&raw).unwrap();
        let decompressed = decompress_segment(&compressed).unwrap();
        assert_eq!(decompressed, raw);
    }

    #[test]
    fn compressed_frame_has_content_checksum_flag() {
        let compressed = compress_segment(b"bencher replica segment").unwrap();
        // zstd frame magic number, little-endian on the wire.
        assert_eq!(&compressed[..4], &[0x28, 0xb5, 0x2f, 0xfd]);
        // Frame_Header_Descriptor bit 2 is the Content_Checksum_flag.
        assert_eq!(compressed[4] & 0b100, 0b100);
    }

    #[test]
    fn zstd_corrupt_payload_detected() {
        let raw = noise_bytes(8 * 1024);
        let mut compressed = compress_segment(&raw).unwrap();
        assert!(compressed.len() > 1024);
        // Flip a byte well past the frame header, in the compressed body.
        compressed[512] ^= 0xff;
        decompress_segment(&compressed).unwrap_err();
    }

    #[test]
    fn zstd_corrupt_checksum_detected() {
        let raw = noise_bytes(4 * 1024);
        let mut compressed = compress_segment(&raw).unwrap();
        // The content checksum is the final 4 bytes of the frame; corrupting
        // it proves the checksum is both present and verified on decode.
        let last = compressed.len() - 1;
        compressed[last] ^= 0xff;
        decompress_segment(&compressed).unwrap_err();
    }

    #[test]
    fn empty_input_roundtrip() {
        let compressed = compress_segment(&[]).unwrap();
        let decompressed = decompress_segment(&compressed).unwrap();
        assert_eq!(decompressed, Vec::<u8>::new());
    }

    #[test]
    fn compression_actually_compresses() {
        let raw = b"bencher replica wal frame ".repeat(40_330);
        assert!(raw.len() > 1024 * 1024);
        let compressed = compress_segment(&raw).unwrap();
        // Repetitive input must shrink by orders of magnitude
        // (multiplication instead of division to keep clippy quiet).
        assert!(compressed.len() * 100 < raw.len());
    }

    #[test]
    fn decompress_rejects_output_beyond_cap() {
        let raw = vec![0u8; 1000];
        let compressed = compress_segment(&raw).unwrap();
        let err = decompress_segment_with_cap(&compressed, 100).unwrap_err();
        assert!(matches!(err, SegmentError::TooLarge { max_bytes: 100 }));
        // At or below the cap decompresses fine.
        let decompressed = decompress_segment_with_cap(&compressed, 1000).unwrap();
        assert_eq!(decompressed, raw);
        assert!(MAX_DECOMPRESSED_BYTES >= u64::from(u32::MAX));
    }

    #[test]
    fn decompress_garbage_is_error() {
        let err = decompress_segment(b"definitely not a zstd frame").unwrap_err();
        assert!(matches!(err, SegmentError::Decompress(_)));
    }

    #[test]
    fn exact_size_cap_rejects_oversize_without_full_allocation() {
        // Restore knows each segment's exact raw size from its key and passes
        // `expected + 1` as the cap, so a corrupt object that decompresses
        // larger fails immediately instead of forcing the generic 4 GiB
        // allocation. A highly compressible body stands in for a zstd bomb.
        let expected = 4096u64;
        let raw = vec![0u8; usize::try_from(expected).unwrap() + 8192];
        let compressed = compress_segment(&raw).unwrap();
        let err = decompress_segment_with_cap(&compressed, expected + 1).unwrap_err();
        assert!(matches!(err, SegmentError::TooLarge { max_bytes } if max_bytes == expected + 1));
        // A body of exactly the expected size decompresses fine at that cap.
        let exact = vec![0u8; usize::try_from(expected).unwrap()];
        let compressed = compress_segment(&exact).unwrap();
        assert_eq!(
            decompress_segment_with_cap(&compressed, expected + 1).unwrap(),
            exact
        );
    }
}
