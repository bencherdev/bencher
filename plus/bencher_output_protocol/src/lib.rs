//! Output file wire protocol for Bencher benchmark VMs.
//!
//! This crate provides pure encode/decode logic for the length-prefixed binary
//! protocol used to transfer output files between the guest VM and the host
//! via vsock.
//!
//! Wire format:
//! ```text
//! [u32 file_count, little-endian]
//! For each file:
//!   [u32 path_len, little-endian]
//!   [path_len bytes of UTF-8 path]
//!   [u64 content_len, little-endian]
//!   [content_len bytes of file content]
//! ```

use camino::{Utf8Path, Utf8PathBuf};

/// Maximum allowed path length in bytes (4 KiB).
///
/// Paths exceeding this limit are rejected during decode to prevent
/// abuse from malformed or malicious data.
pub const MAX_PATH_LENGTH: u32 = 4_096;

/// Decode error.
#[derive(Debug, thiserror::Error)]
#[expect(
    variant_size_differences,
    reason = "UnexpectedEof carries a &'static str (16 bytes) while other variants carry u32 (4 bytes); boxing would add indirection for a rarely-constructed error type"
)]
pub enum DecodeError {
    /// Unexpected end of data while reading a field.
    #[error("unexpected end of data while reading {0}")]
    UnexpectedEof(&'static str),
    /// Invalid UTF-8 in file path.
    #[error("invalid UTF-8 in file path")]
    InvalidUtf8Path,
    /// File count exceeds the configured maximum.
    #[error("file count {0} exceeds maximum")]
    TooManyFiles(u32),
    /// Path length exceeds the maximum allowed length.
    #[error("path length {0} exceeds maximum {MAX_PATH_LENGTH}")]
    PathTooLong(u32),
}

/// Encode error.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// Path length exceeds the maximum allowed length.
    #[error("path length {0} exceeds maximum {MAX_PATH_LENGTH}")]
    PathTooLong(u32),
}

/// Encode file path+content pairs into the length-prefixed binary protocol.
#[expect(
    clippy::little_endian_bytes,
    reason = "wire protocol is defined as little-endian"
)]
pub fn encode(files: &[(&Utf8Path, &[u8])]) -> Result<Vec<u8>, EncodeError> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "file count will not exceed u32"
    )]
    let file_count = files.len() as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&file_count.to_le_bytes());

    for (path, content) in files {
        let path_bytes = path.as_str().as_bytes();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "path length will not exceed u32"
        )]
        let path_len = path_bytes.len() as u32;
        if path_len > MAX_PATH_LENGTH {
            return Err(EncodeError::PathTooLong(path_len));
        }
        buf.extend_from_slice(&path_len.to_le_bytes());
        buf.extend_from_slice(path_bytes);

        let content_len = content.len() as u64;
        buf.extend_from_slice(&content_len.to_le_bytes());
        buf.extend_from_slice(content);
    }

    Ok(buf)
}

/// Decode the binary protocol back into (path, content) pairs.
///
/// `max_file_count` limits how many files may be decoded. If the wire
/// `file_count` exceeds this limit, [`DecodeError::TooManyFiles`] is
/// returned **before** any allocation occurs.
pub fn decode(
    data: &[u8],
    max_file_count: u32,
) -> Result<Vec<(Utf8PathBuf, Vec<u8>)>, DecodeError> {
    let mut cursor = 0;

    let file_count = read_u32(data, &mut cursor, "file count")?;
    if file_count > max_file_count {
        return Err(DecodeError::TooManyFiles(file_count));
    }
    let mut files = Vec::with_capacity(file_count as usize);

    for _ in 0..file_count {
        let path_len = read_u32(data, &mut cursor, "path length")?;
        if path_len > MAX_PATH_LENGTH {
            return Err(DecodeError::PathTooLong(path_len));
        }
        let path_len_usize = path_len as usize;
        if cursor + path_len_usize > data.len() {
            return Err(DecodeError::UnexpectedEof("path"));
        }
        #[expect(clippy::indexing_slicing, reason = "bounds checked above")]
        let path_slice = &data[cursor..cursor + path_len_usize];
        let path_str =
            std::str::from_utf8(path_slice).map_err(|_utf8_err| DecodeError::InvalidUtf8Path)?;
        let path = Utf8PathBuf::from(path_str);
        cursor += path_len_usize;

        #[expect(
            clippy::cast_possible_truncation,
            reason = "content size is bounded by the data slice length"
        )]
        let content_len = read_u64(data, &mut cursor, "content length")? as usize;
        if cursor + content_len > data.len() {
            return Err(DecodeError::UnexpectedEof("content"));
        }
        #[expect(clippy::indexing_slicing, reason = "bounds checked above")]
        let content = data[cursor..cursor + content_len].to_vec();
        cursor += content_len;

        files.push((path, content));
    }

    Ok(files)
}

#[expect(
    clippy::little_endian_bytes,
    reason = "wire protocol is defined as little-endian"
)]
fn read_u32(data: &[u8], cursor: &mut usize, field: &'static str) -> Result<u32, DecodeError> {
    if *cursor + 4 > data.len() {
        return Err(DecodeError::UnexpectedEof(field));
    }
    #[expect(clippy::indexing_slicing, reason = "bounds checked above")]
    let slice = &data[*cursor..*cursor + 4];
    let bytes: [u8; 4] = match slice.try_into() {
        Ok(b) => b,
        Err(_) => return Err(DecodeError::UnexpectedEof(field)),
    };
    *cursor += 4;
    Ok(u32::from_le_bytes(bytes))
}

#[expect(
    clippy::little_endian_bytes,
    reason = "wire protocol is defined as little-endian"
)]
fn read_u64(data: &[u8], cursor: &mut usize, field: &'static str) -> Result<u64, DecodeError> {
    if *cursor + 8 > data.len() {
        return Err(DecodeError::UnexpectedEof(field));
    }
    #[expect(clippy::indexing_slicing, reason = "bounds checked above")]
    let slice = &data[*cursor..*cursor + 8];
    let bytes: [u8; 8] = match slice.try_into() {
        Ok(b) => b,
        Err(_) => return Err(DecodeError::UnexpectedEof(field)),
    };
    *cursor += 8;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::little_endian_bytes,
    clippy::cast_possible_truncation,
    reason = "test code"
)]
mod tests {
    use super::*;

    const TEST_MAX_FILE_COUNT: u32 = 255;

    // --- Existing roundtrip tests ---

    #[test]
    fn roundtrip_empty() {
        let encoded = encode(&[]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert!(decoded.is_empty(), "expected empty vec, got {decoded:?}");
    }

    #[test]
    fn roundtrip_single_file() {
        let path = Utf8Path::new("/tmp/results.json");
        let content = b"benchmark data";
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, path);
        assert_eq!(decoded[0].1, content);
    }

    #[test]
    fn roundtrip_multiple_files() {
        let files: Vec<(&Utf8Path, &[u8])> = vec![
            (Utf8Path::new("/output/a.json"), b"file a content"),
            (Utf8Path::new("/output/b.txt"), b"file b content"),
        ];
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, "/output/a.json");
        assert_eq!(decoded[0].1, b"file a content");
        assert_eq!(decoded[1].0, "/output/b.txt");
        assert_eq!(decoded[1].1, b"file b content");
    }

    #[test]
    fn roundtrip_empty_content() {
        let path = Utf8Path::new("empty.txt");
        let content: &[u8] = b"";
        let encoded = encode(&[(path, content)]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "empty.txt");
        assert!(decoded[0].1.is_empty());
    }

    #[test]
    fn roundtrip_binary_content() {
        let path = Utf8Path::new("binary.bin");
        let content: &[u8] = &[0xFF, 0xFE, 0x00, 0x01, 0x80];
        let encoded = encode(&[(path, content)]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "binary.bin");
        assert_eq!(decoded[0].1, content);
    }

    #[test]
    fn encode_empty_slice_produces_zero_header() {
        let encoded = encode(&[]).unwrap();
        assert_eq!(encoded, 0u32.to_le_bytes());
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert!(decoded.is_empty());
    }

    // --- Existing error tests ---

    #[test]
    fn decode_error_truncated_file_count() {
        // Only 2 bytes instead of 4 for file_count
        let data = &[0x01, 0x00];
        let err = decode(data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("file count"),
            "expected 'file count' in error, got: {err}"
        );
    }

    #[test]
    fn decode_error_truncated_path_length() {
        // file_count = 1, then only 2 bytes for path_len
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x05, 0x00]); // incomplete u32
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("path length"),
            "expected 'path length' in error, got: {err}"
        );
    }

    #[test]
    fn decode_error_truncated_path() {
        // file_count = 1, path_len = 10, but only 3 bytes of path
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&10u32.to_le_bytes());
        data.extend_from_slice(b"abc");
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("path"),
            "expected 'path' in error, got: {err}"
        );
    }

    #[test]
    fn decode_error_truncated_content_length() {
        // file_count = 1, valid path, then only 4 bytes for content_len (needs 8)
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        let path = b"a.txt";
        data.extend_from_slice(&(path.len() as u32).to_le_bytes());
        data.extend_from_slice(path);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // incomplete u64
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("content length"),
            "expected 'content length' in error, got: {err}"
        );
    }

    #[test]
    fn decode_error_invalid_utf8_path() {
        // file_count = 1, path_len = 3, then 3 bytes of invalid UTF-8
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&[0xFF, 0xFE, 0xFD]); // invalid UTF-8
        // Add content_len and content so the only error is the path
        data.extend_from_slice(&0u64.to_le_bytes());
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("invalid UTF-8"),
            "expected 'invalid UTF-8' in error, got: {err}"
        );
    }

    #[test]
    fn decode_error_truncated_content() {
        // file_count = 1, valid path, content_len = 100 but only 5 bytes
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        let path = b"a.txt";
        data.extend_from_slice(&(path.len() as u32).to_le_bytes());
        data.extend_from_slice(path);
        data.extend_from_slice(&100u64.to_le_bytes());
        data.extend_from_slice(b"hello");
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("content"),
            "expected 'content' in error, got: {err}"
        );
    }

    // --- New limit tests ---

    #[test]
    fn decode_error_file_count_exceeds_max() {
        // file_count = 256 with max_file_count = 255
        let mut data = Vec::new();
        data.extend_from_slice(&256u32.to_le_bytes());
        let err = decode(&data, 255).unwrap_err();
        assert!(
            matches!(err, DecodeError::TooManyFiles(256)),
            "expected TooManyFiles(256), got: {err}"
        );
    }

    #[test]
    fn decode_error_path_length_exceeds_max() {
        // file_count = 1, path_len = MAX_PATH_LENGTH + 1
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&(MAX_PATH_LENGTH + 1).to_le_bytes());
        let err = decode(&data, TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            matches!(err, DecodeError::PathTooLong(len) if len == MAX_PATH_LENGTH + 1),
            "expected PathTooLong, got: {err}"
        );
    }

    #[test]
    fn decode_at_max_file_count_boundary() {
        // Encode exactly max_file_count files, should succeed
        let max = 3u32;
        let files: Vec<(&Utf8Path, &[u8])> = vec![
            (Utf8Path::new("a"), b"1"),
            (Utf8Path::new("b"), b"2"),
            (Utf8Path::new("c"), b"3"),
        ];
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, max).unwrap();
        assert_eq!(decoded.len(), 3);
    }

    #[test]
    fn decode_default_max_file_count() {
        // Verify 255 files can be decoded with max_file_count = 255
        let files: Vec<(&Utf8Path, &[u8])> = (0..255u32)
            .map(|i| {
                // Leak the string so we get a &'static str for Utf8Path
                let s: &'static str = Box::leak(format!("f{i}").into_boxed_str());
                (Utf8Path::new(s), &b"x"[..])
            })
            .collect();
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, 255).unwrap();
        assert_eq!(decoded.len(), 255);
    }

    // --- New roundtrip edge cases ---

    #[test]
    fn roundtrip_many_files() {
        let files: Vec<(&Utf8Path, &[u8])> = (0..100u32)
            .map(|i| {
                let s: &'static str = Box::leak(format!("file_{i}.txt").into_boxed_str());
                (Utf8Path::new(s), &b"data"[..])
            })
            .collect();
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 100);
        for (i, (path, content)) in decoded.iter().enumerate() {
            assert_eq!(path.as_str(), format!("file_{i}.txt"));
            assert_eq!(content, b"data");
        }
    }

    #[test]
    fn roundtrip_empty_path() {
        let path = Utf8Path::new("");
        let content = b"content";
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "");
        assert_eq!(decoded[0].1, b"content");
    }

    #[test]
    fn roundtrip_unicode_path() {
        let path = Utf8Path::new("/tmp/résults_日本語.json");
        let content = b"unicode path test";
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "/tmp/résults_日本語.json");
        assert_eq!(decoded[0].1, b"unicode path test");
    }

    #[test]
    fn roundtrip_deeply_nested_path() {
        let path_str = (0..50)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join("/");
        let path = Utf8Path::new(&path_str);
        let content = b"deep";
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0.as_str(), path_str);
        assert_eq!(decoded[0].1, b"deep");
    }

    #[test]
    fn roundtrip_large_content() {
        let path = Utf8Path::new("large.bin");
        let content = vec![0xABu8; 1024 * 1024]; // 1 MiB
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].1.len(), 1024 * 1024);
        assert!(decoded[0].1.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn roundtrip_single_byte_content() {
        let path = Utf8Path::new("tiny.bin");
        let content: &[u8] = &[0x42];
        let encoded = encode(&[(path, content)]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].1, &[0x42]);
    }

    #[test]
    fn roundtrip_duplicate_paths() {
        let files: Vec<(&Utf8Path, &[u8])> = vec![
            (Utf8Path::new("dup.txt"), b"first"),
            (Utf8Path::new("dup.txt"), b"second"),
        ];
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, "dup.txt");
        assert_eq!(decoded[0].1, b"first");
        assert_eq!(decoded[1].0, "dup.txt");
        assert_eq!(decoded[1].1, b"second");
    }

    #[test]
    fn roundtrip_relative_and_absolute_paths() {
        let files: Vec<(&Utf8Path, &[u8])> = vec![
            (Utf8Path::new("./foo/bar.txt"), b"relative"),
            (Utf8Path::new("/tmp/foo/bar.txt"), b"absolute"),
        ];
        let encoded = encode(&files).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, "./foo/bar.txt");
        assert_eq!(decoded[1].0, "/tmp/foo/bar.txt");
    }

    // --- Wire format verification ---

    #[test]
    fn encode_wire_format_single_file() {
        let path = Utf8Path::new("a.txt");
        let content = b"hi";
        let encoded = encode(&[(path, content.as_slice())]).unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(&1u32.to_le_bytes()); // file_count = 1
        expected.extend_from_slice(&5u32.to_le_bytes()); // path_len = 5
        expected.extend_from_slice(b"a.txt"); // path
        expected.extend_from_slice(&2u64.to_le_bytes()); // content_len = 2
        expected.extend_from_slice(b"hi"); // content

        assert_eq!(encoded, expected);
    }

    #[test]
    fn decode_hand_crafted_valid_message() {
        let mut data = Vec::new();
        data.extend_from_slice(&2u32.to_le_bytes()); // file_count = 2

        // File 1: "x" with content "AB"
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(b"x");
        data.extend_from_slice(&2u64.to_le_bytes());
        data.extend_from_slice(b"AB");

        // File 2: "yy" with content "C"
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(b"yy");
        data.extend_from_slice(&1u64.to_le_bytes());
        data.extend_from_slice(b"C");

        let decoded = decode(&data, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].0, "x");
        assert_eq!(decoded[0].1, b"AB");
        assert_eq!(decoded[1].0, "yy");
        assert_eq!(decoded[1].1, b"C");
    }

    // --- Boundary/edge cases ---

    #[test]
    fn decode_completely_empty_buffer() {
        let err = decode(&[], TEST_MAX_FILE_COUNT).unwrap_err();
        assert!(
            err.to_string().contains("file count"),
            "expected 'file count' in error, got: {err}"
        );
    }

    #[test]
    fn decode_trailing_bytes_after_valid_data() {
        let encoded = encode(&[(Utf8Path::new("f"), &b"d"[..])]).unwrap();
        let mut with_trailing = encoded.clone();
        with_trailing.extend_from_slice(b"garbage");
        // Trailing bytes are silently ignored
        let decoded = decode(&with_trailing, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "f");
        assert_eq!(decoded[0].1, b"d");
    }

    #[test]
    fn decode_file_count_zero_with_extra_data() {
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"extra bytes");
        let decoded = decode(&data, TEST_MAX_FILE_COUNT).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn roundtrip_all_zero_content() {
        let path = Utf8Path::new("zeros.bin");
        let content = vec![0u8; 256];
        let encoded = encode(&[(path, content.as_slice())]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].1, content);
    }

    #[test]
    fn encode_error_path_too_long() {
        let long_path = "a".repeat((MAX_PATH_LENGTH + 1) as usize);
        let path = Utf8Path::new(&long_path);
        let err = encode(&[(path, b"data")]).unwrap_err();
        assert!(
            matches!(err, EncodeError::PathTooLong(len) if len == MAX_PATH_LENGTH + 1),
            "expected PathTooLong, got: {err}"
        );
    }

    #[test]
    fn roundtrip_content_with_embedded_nulls() {
        let path = Utf8Path::new("mixed.bin");
        let content: &[u8] = b"hello\x00world\x00\x00end";
        let encoded = encode(&[(path, content)]).unwrap();
        let decoded = decode(&encoded, TEST_MAX_FILE_COUNT).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].1, content);
    }
}
