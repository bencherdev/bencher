//! The record a guest leaves on its results drive before it powers off, which
//! the host reads only once the guest is gone.
//!
//! ```text
//! [4 KiB header at offset 0]
//!   [8 bytes magic "BNCHRES1"]
//!   [i32 exit code, little-endian]
//!   [4 zero bytes]
//!   [u64 stdout length, little-endian]
//!   [u64 stderr length, little-endian]
//!   [u64 output files length, little-endian]
//!   [zero bytes to 4 KiB]
//! [stdout][stderr][output files], from offset 4 KiB
//! ```
//!
//! The output files are in the length-prefixed protocol of this crate's root.

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

const MAGIC: [u8; 8] = *b"BNCHRES1";

/// Also where the payload starts.
const HEADER_LEN: usize = 4096;

/// A drive's size is a whole number of these, which a block device needs.
const BLOCK: u64 = 4096;

/// What a guest leaves for the host, each field at most the host's cap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// In the length-prefixed protocol, empty for none.
    pub output_files: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("the record's {field} is {len} bytes, over its cap")]
    OverCap { field: &'static str, len: u64 },
    #[error("the drive ends inside the record")]
    Truncated,
    #[error("failed to read the record: {0}")]
    Io(std::io::Error),
}

/// The size of a drive that holds a record whose every field is at `cap`, or
/// `None` past `u64`.
pub fn drive_size(cap: u64) -> Option<u64> {
    cap.checked_mul(3)?
        .checked_add(HEADER_LEN as u64)?
        .checked_next_multiple_of(BLOCK)
}

/// The payload first, then the header, each flushed, so a header on the drive
/// always has its whole payload behind it.
pub fn write(drive: &mut File, record: &Record) -> std::io::Result<()> {
    let header = Header {
        exit_code: record.exit_code,
        lengths: [&record.stdout, &record.stderr, &record.output_files]
            .map(|field| field.len() as u64),
    };
    for ((offset, _len), field) in
        header
            .regions()
            .into_iter()
            .zip([&record.stdout, &record.stderr, &record.output_files])
    {
        drive.seek(SeekFrom::Start(offset))?;
        drive.write_all(field)?;
    }
    drive.sync_all()?;
    drive.seek(SeekFrom::Start(0))?;
    drive.write_all(&header.encode())?;
    drive.sync_all()
}

/// `Ok(None)` is a drive with no record. Every field is checked against `cap`
/// before it is read, so nothing past the drive the host sized for `cap` is.
pub fn read(drive: &File, cap: u64) -> Result<Option<Record>, ReadError> {
    let mut drive = drive;
    let mut bytes = [0u8; HEADER_LEN];
    read_at(&mut drive, 0, &mut bytes)?;
    let Some(header) = Header::decode(&bytes) else {
        return Ok(None);
    };
    for (field, len) in FIELDS.into_iter().zip(header.lengths) {
        if len > cap {
            return Err(ReadError::OverCap { field, len });
        }
    }
    let [stdout, stderr, output_files] = header.regions().map(|(offset, len)| {
        let len = usize::try_from(len).map_err(|_err| ReadError::Truncated)?;
        let mut field = vec![0u8; len];
        read_at(&mut drive, offset, &mut field)?;
        Ok(field)
    });
    Ok(Some(Record {
        exit_code: header.exit_code,
        stdout: stdout?,
        stderr: stderr?,
        output_files: output_files?,
    }))
}

const FIELDS: [&str; 3] = ["stdout", "stderr", "output files"];

struct Header {
    exit_code: i32,
    /// Stdout, stderr, and the output files, in the order they sit on the drive.
    lengths: [u64; 3],
}

impl Header {
    /// Each field's offset and length, saturating so that no length wraps an
    /// offset back onto the drive.
    fn regions(&self) -> [(u64, u64); 3] {
        let mut offset = HEADER_LEN as u64;
        self.lengths.map(|len| {
            let region = (offset, len);
            offset = offset.saturating_add(len);
            region
        })
    }

    #[expect(
        clippy::little_endian_bytes,
        reason = "the record is little-endian, like the output file protocol"
    )]
    fn encode(&self) -> [u8; HEADER_LEN] {
        let [stdout, stderr, output_files] = self.lengths.map(u64::to_le_bytes);
        let fields: [&[u8]; 6] = [
            &MAGIC,
            &self.exit_code.to_le_bytes(),
            &[0; 4],
            &stdout,
            &stderr,
            &output_files,
        ];
        let mut bytes = [0u8; HEADER_LEN];
        for (byte, value) in bytes.iter_mut().zip(fields.into_iter().flatten()) {
            *byte = *value;
        }
        bytes
    }

    /// `None` without the magic, which is how a drive the guest never wrote
    /// reads.
    #[expect(
        clippy::little_endian_bytes,
        reason = "the record is little-endian, like the output file protocol"
    )]
    fn decode(bytes: &[u8; HEADER_LEN]) -> Option<Self> {
        let (magic, rest) = bytes.split_first_chunk::<8>()?;
        if *magic != MAGIC {
            return None;
        }
        let (exit_code, rest) = rest.split_first_chunk::<4>()?;
        let (_zero, rest) = rest.split_first_chunk::<4>()?;
        let (stdout, rest) = rest.split_first_chunk::<8>()?;
        let (stderr, rest) = rest.split_first_chunk::<8>()?;
        let (output_files, _rest) = rest.split_first_chunk::<8>()?;
        Some(Self {
            exit_code: i32::from_le_bytes(*exit_code),
            lengths: [stdout, stderr, output_files].map(|len| u64::from_le_bytes(*len)),
        })
    }
}

fn read_at(drive: &mut &File, offset: u64, buf: &mut [u8]) -> Result<(), ReadError> {
    drive.seek(SeekFrom::Start(offset)).map_err(ReadError::Io)?;
    drive.read_exact(buf).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            ReadError::Truncated
        } else {
            ReadError::Io(e)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: u64 = 64 * 1024;

    fn record() -> Record {
        Record {
            exit_code: -7,
            stdout: b"out".repeat(1000),
            stderr: b"err".repeat(10),
            output_files: crate::encode(&[(camino::Utf8Path::new("/a.json"), b"{}".as_slice())])
                .unwrap(),
        }
    }

    /// A drive sized for `cap`, as the host makes it.
    fn drive(cap: u64) -> (tempfile::TempDir, std::path::PathBuf, File) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.img");
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.set_len(drive_size(cap).unwrap()).unwrap();
        (dir, path, file)
    }

    /// The header as a guest that skipped `write`'s checks would leave it.
    fn header(exit_code: i32, lengths: [u64; 3]) -> [u8; HEADER_LEN] {
        Header { exit_code, lengths }.encode()
    }

    fn put(file: &mut File, offset: u64, bytes: &[u8]) {
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(bytes).unwrap();
    }

    #[test]
    fn a_record_reads_back_as_it_was_written() {
        // Kills a field read from another field's offset, and a field order
        // that differs between the writer and the reader.
        let (_dir, _path, mut file) = drive(CAP);

        write(&mut file, &record()).unwrap();

        assert_eq!(read(&file, CAP).unwrap(), Some(record()));
    }

    #[test]
    fn a_record_of_fields_at_their_cap_fits_the_drive() {
        // Kills a drive sized for fewer full fields than the record has.
        let full = Record {
            exit_code: 0,
            stdout: vec![b'o'; 4096 + 1],
            stderr: vec![b'e'; 4096 + 1],
            output_files: vec![b'f'; 4096 + 1],
        };
        let cap = 4096 + 1;
        let (_dir, path, mut file) = drive(cap);

        write(&mut file, &full).unwrap();

        assert_eq!(read(&file, cap).unwrap(), Some(full));
        assert_eq!(
            std::fs::metadata(path).unwrap().len(),
            drive_size(cap).unwrap(),
            "writing a full record must not grow the drive"
        );
    }

    #[test]
    fn a_drive_is_a_whole_number_of_blocks() {
        // Kills a size a block device would cut short, losing the record's tail.
        for cap in [0, 1, 1024, 4096, 25 * 1024 * 1024 + 3] {
            let size = drive_size(cap).unwrap();
            assert!(
                size.is_multiple_of(BLOCK) && size >= 3 * cap + HEADER_LEN as u64,
                "cap {cap}: size {size}"
            );
        }
    }

    #[test]
    fn a_cap_too_large_to_size_a_drive_for_is_refused() {
        // Kills arithmetic that wraps to a small drive.
        assert_eq!(drive_size(1 << 63), None);
        assert_eq!(drive_size(u64::MAX), None);
    }

    #[test]
    fn a_drive_the_guest_never_wrote_holds_no_record() {
        // Kills reading an all-zero header as a record of exit code 0 and no
        // output, which reports a guest that died early as a success.
        let (_dir, _path, file) = drive(CAP);

        assert_eq!(read(&file, CAP).unwrap(), None);
    }

    #[test]
    fn a_header_without_the_whole_magic_holds_no_record() {
        // Kills a magic check that stops short of the last byte.
        let (_dir, _path, mut file) = drive(CAP);
        let mut bytes = header(0, [0, 0, 0]);
        bytes[7] = b'2';
        put(&mut file, 0, &bytes);

        assert_eq!(read(&file, CAP).unwrap(), None);
    }

    #[test]
    fn a_field_over_its_cap_is_refused_before_it_is_read() {
        // Kills a missing or off-by-one cap check on any field: the drive is
        // large enough that an unchecked read would succeed.
        let (_dir, _path, mut file) = drive(4 * CAP);
        for (index, field) in FIELDS.into_iter().enumerate() {
            let mut lengths = [0; 3];
            lengths[index] = CAP;
            put(&mut file, 0, &header(0, lengths));
            read(&file, CAP).unwrap().unwrap();

            lengths[index] = CAP + 1;
            put(&mut file, 0, &header(0, lengths));
            let err = read(&file, CAP).unwrap_err();
            assert!(
                matches!(err, ReadError::OverCap { field: f, len } if f == field && len == CAP + 1),
                "{field}: {err}"
            );
        }
    }

    #[test]
    fn a_drive_cut_short_under_its_record_is_refused() {
        // Kills accepting a short read, which hands on zeros or a partial field
        // as the guest's output.
        let (_dir, _path, mut file) = drive(CAP);
        write(&mut file, &record()).unwrap();
        file.set_len(HEADER_LEN as u64 + 10).unwrap();

        let err = read(&file, CAP).unwrap_err();

        assert!(matches!(err, ReadError::Truncated), "{err}");
    }

    #[test]
    fn a_drive_cut_short_inside_its_header_is_refused() {
        let (_dir, _path, file) = drive(CAP);
        file.set_len(10).unwrap();

        let err = read(&file, CAP).unwrap_err();

        assert!(matches!(err, ReadError::Truncated), "{err}");
    }
}
