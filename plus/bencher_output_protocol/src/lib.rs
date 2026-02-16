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

/// Decode error.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Unexpected end of data while reading a field.
    #[error("unexpected end of data while reading {0}")]
    UnexpectedEof(&'static str),
}

/// Encode file path+content pairs into the length-prefixed binary protocol.
#[expect(
    clippy::little_endian_bytes,
    reason = "wire protocol is defined as little-endian"
)]
pub fn encode(files: &[(&Utf8Path, &[u8])]) -> Vec<u8> {
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
        buf.extend_from_slice(&path_len.to_le_bytes());
        buf.extend_from_slice(path_bytes);

        let content_len = content.len() as u64;
        buf.extend_from_slice(&content_len.to_le_bytes());
        buf.extend_from_slice(content);
    }

    buf
}

/// Decode the binary protocol back into (path, content) pairs.
pub fn decode(data: &[u8]) -> Result<Vec<(Utf8PathBuf, Vec<u8>)>, DecodeError> {
    let mut cursor = 0;

    let file_count = read_u32(data, &mut cursor, "file count")?;
    let mut files = Vec::with_capacity(file_count as usize);

    for _ in 0..file_count {
        let path_len = read_u32(data, &mut cursor, "path length")? as usize;
        if cursor + path_len > data.len() {
            return Err(DecodeError::UnexpectedEof("path"));
        }
        #[expect(clippy::indexing_slicing, reason = "bounds checked above")]
        let path_slice = &data[cursor..cursor + path_len];
        let path = Utf8PathBuf::from(String::from_utf8_lossy(path_slice).as_ref());
        cursor += path_len;

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

    #[test]
    fn roundtrip_empty() {
        let encoded = encode(&[]);
        let decoded = decode(&encoded).unwrap();
        assert!(decoded.is_empty(), "expected empty vec, got {decoded:?}");
    }

    #[test]
    fn roundtrip_single_file() {
        let path = Utf8Path::new("/tmp/results.json");
        let content = b"benchmark data";
        let encoded = encode(&[(path, content.as_slice())]);
        let decoded = decode(&encoded).unwrap();
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
        let encoded = encode(&files);
        let decoded = decode(&encoded).unwrap();
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
        let encoded = encode(&[(path, content)]);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "empty.txt");
        assert!(decoded[0].1.is_empty());
    }

    #[test]
    fn roundtrip_binary_content() {
        let path = Utf8Path::new("binary.bin");
        let content: &[u8] = &[0xFF, 0xFE, 0x00, 0x01, 0x80];
        let encoded = encode(&[(path, content)]);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "binary.bin");
        assert_eq!(decoded[0].1, content);
    }

    #[test]
    fn encode_empty_slice_produces_zero_header() {
        let encoded = encode(&[]);
        assert_eq!(encoded, 0u32.to_le_bytes());
        let decoded = decode(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_error_truncated_file_count() {
        // Only 2 bytes instead of 4 for file_count
        let data = &[0x01, 0x00];
        let err = decode(data).unwrap_err();
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
        let err = decode(&data).unwrap_err();
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
        let err = decode(&data).unwrap_err();
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
        let err = decode(&data).unwrap_err();
        assert!(
            err.to_string().contains("content length"),
            "expected 'content length' in error, got: {err}"
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
        let err = decode(&data).unwrap_err();
        assert!(
            err.to_string().contains("content"),
            "expected 'content' in error, got: {err}"
        );
    }
}
