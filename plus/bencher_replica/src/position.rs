//! Replica addressing: generations, epochs, and object key naming.
//!
//! Replica layout (all keys relative to the configured root/prefix):
//!
//! ```text
//! generations/
//!   20260710T145900Z-3f8a2c1d/            # GenerationId; lexicographic order == age
//!     snapshot.db.zst
//!     snapshot.json                        # atomic "generation is valid" marker
//!     wal/
//!       0000000000-9d2f1c4a8b3e6f70/       # <epoch:010>-<salt1:08x><salt2:08x>
//!         00000000000000000000-00000000000524320.wal.zst
//! ```
//!
//! Key-naming rules pinned here (heavily unit-tested, parsed by LIST-driven
//! resume and restore):
//!
//! - Generation IDs are `%Y%m%dT%H%M%SZ-<8 lowercase hex>`: lexicographic
//!   order equals creation order (single process; random suffix breaks
//!   same-second ties).
//! - Epoch directories embed the WAL salts so resume can compare the local
//!   WAL header against the replica from a LIST alone.
//! - Segment names are `<start:020>-<end:020>.wal.zst` with zero-padded
//!   decimal byte offsets: lexicographic order equals numeric order, and the
//!   shipped position is derivable from the last key alone.
//! - The first segment of every epoch starts at offset 0 and therefore
//!   contains the 32-byte WAL header: restore rebuilds a `-wal` file by
//!   decompress-and-concatenate.

use bencher_json::DateTime;

/// Prefix under which all generations live.
pub const GENERATIONS_PREFIX: &str = "generations";
/// Snapshot object file name within a generation.
pub const SNAPSHOT_FILE: &str = "snapshot.db.zst";
/// Snapshot metadata (generation commit marker) file name.
pub const SNAPSHOT_META_FILE: &str = "snapshot.json";
/// Directory under a generation holding the WAL epoch directories.
pub const WAL_DIR: &str = "wal";

/// A replica generation identifier: `%Y%m%dT%H%M%SZ-<8 hex>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationId(String);

impl GenerationId {
    /// Build a generation ID from a timestamp and an explicit 32-bit suffix
    /// (rendered as 8 lowercase hex digits). Deterministic for tests.
    #[must_use]
    pub fn new(created: DateTime, suffix: u32) -> Self {
        let timestamp = created.into_inner().format(TIMESTAMP_FORMAT);
        Self(format!("{timestamp}-{suffix:08x}"))
    }

    /// Build a generation ID with a random suffix (the leading 32 random bits
    /// of a v4 UUID).
    #[must_use]
    pub fn generate(created: DateTime) -> Self {
        let (suffix, ..) = uuid::Uuid::new_v4().as_fields();
        Self::new(created, suffix)
    }

    /// Parse a generation ID from a single path component; `None` when the
    /// component does not match the expected shape.
    #[must_use]
    pub fn parse(component: &str) -> Option<Self> {
        let (timestamp, suffix) = component.split_once('-')?;
        validate_utc_second(timestamp)?;
        parse_hex_u32(suffix)?;
        Some(Self(component.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lexicographically NEXT id with the same timestamp: suffix plus
    /// one. `None` when the suffix is saturated (2^32 - 1). Used as the
    /// monotonic fallback when the clock has rewound behind an observed
    /// generation: new ids must always sort after the replica tip or restore
    /// would silently pick the older lineage.
    #[must_use]
    pub fn successor(&self) -> Option<Self> {
        let (timestamp, suffix) = self.0.split_once('-')?;
        let next = parse_hex_u32(suffix)?.checked_add(1)?;
        Some(Self(format!("{timestamp}-{next:08x}")))
    }
}

/// In-memory replication position: the next unshipped byte of the WAL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub generation: GenerationId,
    /// WAL salt cycle, numbered from 0 within the generation.
    pub epoch: u64,
    /// Header salts of this epoch.
    pub salt: (u32, u32),
    /// Next unshipped byte offset (commit-aligned; 0 means the WAL header
    /// itself has not shipped yet).
    pub offset: u64,
    /// Running WAL checksum at `offset` (chain continuation seed).
    pub checksum: (u32, u32),
}

/// Parsed segment key components within a generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentKey {
    pub epoch: u64,
    pub salt: (u32, u32),
    /// Raw WAL byte range `[start, end)` covered by the segment.
    pub start: u64,
    pub end: u64,
}

/// `generations/<id>/` (trailing slash included).
#[must_use]
pub fn generation_prefix(generation: &GenerationId) -> String {
    format!("{GENERATIONS_PREFIX}/{}/", generation.as_str())
}

/// `generations/<id>/snapshot.db.zst`
#[must_use]
pub fn snapshot_key(generation: &GenerationId) -> String {
    format!("{}{SNAPSHOT_FILE}", generation_prefix(generation))
}

/// `generations/<id>/snapshot.json`
#[must_use]
pub fn snapshot_meta_key(generation: &GenerationId) -> String {
    format!("{}{SNAPSHOT_META_FILE}", generation_prefix(generation))
}

/// `generations/<id>/wal/<epoch:010>-<salt1:08x><salt2:08x>/` (trailing slash
/// included).
///
/// Epochs at or above `10^EPOCH_WIDTH` would render wider than the fixed
/// width, breaking lexicographic-equals-numeric ordering, and
/// `parse_epoch_dir` would then reject the directory on restore. Unreachable
/// in practice (the epoch resets per generation and increments once per WAL
/// restart), so it is a debug assertion rather than a runtime error.
#[must_use]
pub fn epoch_dir(generation: &GenerationId, epoch: u64, salt: (u32, u32)) -> String {
    debug_assert!(
        epoch < 10u64.pow(u32::try_from(EPOCH_WIDTH).unwrap_or(u32::MAX)),
        "epoch {epoch} exceeds the fixed {EPOCH_WIDTH}-digit directory width"
    );
    let (salt1, salt2) = salt;
    format!(
        "{}{WAL_DIR}/{epoch:010}-{salt1:08x}{salt2:08x}/",
        generation_prefix(generation)
    )
}

/// Full segment key:
/// `generations/<id>/wal/<epoch dir>/<start:020>-<end:020>.wal.zst`
#[must_use]
pub fn segment_key(generation: &GenerationId, segment: &SegmentKey) -> String {
    let SegmentKey {
        epoch,
        salt,
        start,
        end,
    } = *segment;
    format!(
        "{}{start:020}-{end:020}{SEGMENT_SUFFIX}",
        epoch_dir(generation, epoch, salt)
    )
}

/// Parse an epoch directory component (`<epoch:010>-<salt1:08x><salt2:08x>`);
/// `None` when malformed.
#[must_use]
pub fn parse_epoch_dir(component: &str) -> Option<(u64, (u32, u32))> {
    let (epoch, hex) = component.split_once('-')?;
    if epoch.len() != EPOCH_WIDTH || !epoch.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (salt1, salt2) = hex.split_at_checked(SALT_HEX_WIDTH)?;
    Some((
        epoch.parse().ok()?,
        (parse_hex_u32(salt1)?, parse_hex_u32(salt2)?),
    ))
}

/// Parse a full segment key back into its generation and components; `None`
/// when any component is malformed (including empty or inverted ranges: a
/// real segment always covers at least one byte).
#[must_use]
pub fn parse_segment_key(key: &str) -> Option<(GenerationId, SegmentKey)> {
    let mut parts = key.split('/');
    let prefix = parts.next()?;
    let generation = parts.next()?;
    let wal = parts.next()?;
    let epoch_component = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() || prefix != GENERATIONS_PREFIX || wal != WAL_DIR {
        return None;
    }
    let generation = GenerationId::parse(generation)?;
    let (epoch, salt) = parse_epoch_dir(epoch_component)?;
    let (start, end) = parse_segment_file(file)?;
    Some((
        generation,
        SegmentKey {
            epoch,
            salt,
            start,
            end,
        },
    ))
}

/// `chrono` strftime format behind [`GenerationId`] timestamps.
const TIMESTAMP_FORMAT: &str = "%Y%m%dT%H%M%SZ";
/// Rendered length of [`TIMESTAMP_FORMAT`].
const TIMESTAMP_LEN: usize = 16;
/// Zero-padded decimal width of an epoch number in an epoch directory.
const EPOCH_WIDTH: usize = 10;
/// Lowercase hex width of a single WAL salt in an epoch directory.
const SALT_HEX_WIDTH: usize = 8;
/// Zero-padded decimal width of a segment byte offset (full `u64` range).
const OFFSET_WIDTH: usize = 20;
/// Segment object file name suffix.
const SEGMENT_SUFFIX: &str = ".wal.zst";

/// Validate a `%Y%m%dT%H%M%SZ` timestamp component; `Option<()>` purely for
/// `?` chaining.
fn validate_utc_second(timestamp: &str) -> Option<()> {
    if timestamp.len() != TIMESTAMP_LEN {
        return None;
    }
    let (year, rest) = split_digits(timestamp, 4)?;
    let (month, rest) = split_digits(rest, 2)?;
    let (day, rest) = split_digits(rest, 2)?;
    let rest = rest.strip_prefix('T')?;
    let (hour, rest) = split_digits(rest, 2)?;
    let (minute, rest) = split_digits(rest, 2)?;
    let (second, rest) = split_digits(rest, 2)?;
    // An out-of-range month has zero days, so the day check rejects it too.
    let valid = rest == "Z"
        && (1..=days_in_month(year, month)).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 59;
    valid.then_some(())
}

/// Split `len` leading ASCII digits off `component` and parse them.
fn split_digits(component: &str, len: usize) -> Option<(u32, &str)> {
    let (digits, rest) = component.split_at_checked(len)?;
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, rest))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Parse exactly [`SALT_HEX_WIDTH`] lowercase hex digits (uppercase rejected
/// so every key has one canonical spelling).
fn parse_hex_u32(hex: &str) -> Option<u32> {
    if hex.len() != SALT_HEX_WIDTH || !hex.bytes().all(is_lower_hex_digit) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn is_lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// Parse a segment file name (`<start:020>-<end:020>.wal.zst`) into its
/// `[start, end)` range; requires `start < end`.
fn parse_segment_file(file: &str) -> Option<(u64, u64)> {
    let range = file.strip_suffix(SEGMENT_SUFFIX)?;
    let (start, rest) = range.split_at_checked(OFFSET_WIDTH)?;
    let end = rest.strip_prefix('-')?;
    let start = parse_padded_u64(start)?;
    let end = parse_padded_u64(end)?;
    (start < end).then_some((start, end))
}

/// Parse exactly [`OFFSET_WIDTH`] zero-padded decimal digits; values beyond
/// `u64::MAX` fail the inner parse.
fn parse_padded_u64(digits: &str) -> Option<u64> {
    if digits.len() != OFFSET_WIDTH || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use bencher_json::DateTime;
    use pretty_assertions::assert_eq;

    use super::{
        GenerationId, SegmentKey, epoch_dir, generation_prefix, parse_epoch_dir, parse_segment_key,
        segment_key, snapshot_key, snapshot_meta_key,
    };

    /// 2026-07-10T14:59:00Z
    const CREATED_SECS: i64 = 1_783_695_540;
    const SUFFIX: u32 = 0x3f8a_2c1d;
    const SALT: (u32, u32) = (0x9d2f_1c4a, 0x8b3e_6f70);

    fn created() -> DateTime {
        DateTime::try_from(CREATED_SECS).unwrap()
    }

    fn generation() -> GenerationId {
        GenerationId::new(created(), SUFFIX)
    }

    #[test]
    fn generation_id_new_renders_timestamp_and_suffix() {
        assert_eq!(generation().as_str(), "20260710T145900Z-3f8a2c1d");
        // The suffix is zero-padded, lowercase hex.
        assert_eq!(
            GenerationId::new(created(), 0xa).as_str(),
            "20260710T145900Z-0000000a"
        );
    }

    #[test]
    fn generation_id_parse_round_trips() {
        let generation = generation();
        assert_eq!(
            GenerationId::parse(generation.as_str()),
            Some(generation.clone())
        );
        assert_eq!(GenerationId::parse(generation.as_str()), Some(generation));
    }

    #[test]
    fn generation_id_generate_keeps_timestamp_and_parses() {
        let generated = GenerationId::generate(created());
        assert!(
            generated.as_str().starts_with("20260710T145900Z-"),
            "{generated:?}"
        );
        assert_eq!(GenerationId::parse(generated.as_str()), Some(generated));
    }

    #[test]
    fn generation_id_parse_rejects_malformed() {
        let malformed = [
            "",
            // Missing separator or suffix
            "20260710T145900Z",
            "20260710T145900Z-",
            // Wrong lengths
            "20260710T145900Z-3f8a2c1",
            "20260710T145900Z-3f8a2c1d1",
            "2026710T145900Z-3f8a2c1d",
            "202607100T145900Z-3f8a2c1d",
            // Bad timestamps
            "20261310T145900Z-3f8a2c1d",
            "20260732T145900Z-3f8a2c1d",
            "20260700T145900Z-3f8a2c1d",
            "20260710T245900Z-3f8a2c1d",
            "20260710T146000Z-3f8a2c1d",
            "20260710T145960Z-3f8a2c1d",
            "20260710X145900Z-3f8a2c1d",
            "20260710T145900X-3f8a2c1d",
            "20260710t145900Z-3f8a2c1d",
            "20260710T145900z-3f8a2c1d",
            "2026a710T145900Z-3f8a2c1d",
            // Non-hex or uppercase-hex suffixes
            "20260710T145900Z-3f8a2c1g",
            "20260710T145900Z-3F8A2C1D",
            "20260710T145900Z-3f8a-c1d",
            "20260710T145900Z-+f8a2c1d",
            // Trailing garbage
            "20260710T145900Z-3f8a2c1d/",
            " 20260710T145900Z-3f8a2c1d",
        ];
        for component in malformed {
            assert_eq!(GenerationId::parse(component), None, "{component:?}");
        }
    }

    #[test]
    fn generation_id_parse_handles_leap_years() {
        // 2024 is a leap year; 2023 is not; 1900 is not (century rule);
        // 2000 is (400-year rule).
        assert!(GenerationId::parse("20240229T000000Z-00000000").is_some());
        assert_eq!(GenerationId::parse("20230229T000000Z-00000000"), None);
        assert_eq!(GenerationId::parse("19000229T000000Z-00000000"), None);
        assert!(GenerationId::parse("20000229T000000Z-00000000").is_some());
    }

    #[test]
    fn generation_id_orders_by_timestamp_then_suffix() {
        let earlier = DateTime::try_from(CREATED_SECS).unwrap();
        let later = DateTime::try_from(CREATED_SECS + 1).unwrap();
        // Timestamp dominates even when the earlier suffix is numerically larger.
        assert!(GenerationId::new(earlier, u32::MAX) < GenerationId::new(later, 0));
        // Same second: suffix order is numeric because the hex is fixed-width.
        assert!(GenerationId::new(earlier, 0x1) < GenerationId::new(earlier, 0xa));
        assert!(GenerationId::new(earlier, 0xa) < GenerationId::new(earlier, 0x10));
        assert!(GenerationId::new(earlier, 0x10) < GenerationId::new(earlier, u32::MAX));
    }

    #[test]
    fn generation_id_successor_increments_suffix_and_saturates() {
        // A normal suffix increments by one, carrying across the hex-width
        // boundary, and the result sorts strictly after the original.
        let base = GenerationId::new(created(), 0x0000_000f);
        let next = base
            .successor()
            .expect("successor of a non-saturated suffix");
        assert_eq!(next.as_str(), "20260710T145900Z-00000010");
        assert!(next > base, "the successor sorts after the original");
        // The maximum suffix has no successor (would overflow the 8-hex field).
        assert_eq!(GenerationId::new(created(), u32::MAX).successor(), None);
        // One below the maximum still has a successor: the field saturates.
        let penultimate = GenerationId::new(created(), u32::MAX - 1);
        assert_eq!(
            penultimate.successor().map(|id| id.as_str().to_owned()),
            Some("20260710T145900Z-ffffffff".to_owned())
        );
    }

    #[test]
    fn generation_and_snapshot_keys_render_exactly() {
        let generation = generation();
        assert_eq!(
            generation_prefix(&generation),
            "generations/20260710T145900Z-3f8a2c1d/"
        );
        assert_eq!(
            snapshot_key(&generation),
            "generations/20260710T145900Z-3f8a2c1d/snapshot.db.zst"
        );
        assert_eq!(
            snapshot_meta_key(&generation),
            "generations/20260710T145900Z-3f8a2c1d/snapshot.json"
        );
    }

    #[test]
    fn epoch_dir_renders_zero_padded_salts() {
        assert_eq!(
            epoch_dir(&generation(), 0, SALT),
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/"
        );
        assert_eq!(
            epoch_dir(&generation(), 42, (0xa, 0xb)),
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000042-0000000a0000000b/"
        );
    }

    #[test]
    fn segment_key_renders_zero_padded_offsets() {
        let segment = SegmentKey {
            epoch: 0,
            salt: SALT,
            start: 0,
            end: 524_320,
        };
        assert_eq!(
            segment_key(&generation(), &segment),
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000524320.wal.zst"
        );
    }

    #[test]
    fn segment_key_is_idempotent_and_collision_free() {
        let generation = generation();
        let segment = SegmentKey {
            epoch: 1,
            salt: SALT,
            start: 32,
            end: 4128,
        };
        // Same range yields the same key (idempotent re-upload target).
        assert_eq!(
            segment_key(&generation, &segment),
            segment_key(&generation, &segment)
        );
        // Any differing component yields a different key.
        let variants = [
            SegmentKey {
                start: 4128,
                end: 8224,
                ..segment
            },
            SegmentKey {
                end: 8224,
                ..segment
            },
            SegmentKey {
                epoch: 2,
                ..segment
            },
            SegmentKey {
                salt: (0x1, 0x2),
                ..segment
            },
        ];
        for variant in variants {
            assert_ne!(
                segment_key(&generation, &segment),
                segment_key(&generation, &variant),
                "{variant:?}"
            );
        }
    }

    #[test]
    fn lexicographic_order_equals_numeric_order() {
        let generation = generation();
        // Epoch directories across decimal-width boundaries.
        let epoch_pairs = [
            (9u64, 10u64),
            (99, 100),
            (999_999_999, 1_000_000_000),
            (9_999_999_998, 9_999_999_999),
        ];
        for (lo, hi) in epoch_pairs {
            let lo_dir = epoch_dir(&generation, lo, SALT);
            let hi_dir = epoch_dir(&generation, hi, SALT);
            assert!(lo_dir < hi_dir, "{lo_dir} vs {hi_dir}");
        }
        // Segment offsets across decimal-width boundaries up to u64::MAX.
        let offset_pairs = [(9u64, 10u64), (99, 100), (u64::MAX - 2, u64::MAX - 1)];
        for (lo, hi) in offset_pairs {
            let lo_key = segment_key(
                &generation,
                &SegmentKey {
                    epoch: 0,
                    salt: SALT,
                    start: lo,
                    end: lo + 1,
                },
            );
            let hi_key = segment_key(
                &generation,
                &SegmentKey {
                    epoch: 0,
                    salt: SALT,
                    start: hi,
                    end: hi + 1,
                },
            );
            assert!(lo_key < hi_key, "{lo_key} vs {hi_key}");
        }
        // Generation IDs across second/day/month/year rollovers.
        let time_pairs = [
            ("20261231T235959Z", "20270101T000000Z"),
            ("20270101T000000Z", "20270101T000001Z"),
            ("20270101T000001Z", "20270102T000000Z"),
            ("20270102T000000Z", "20270201T000000Z"),
        ];
        for (lo, hi) in time_pairs {
            let lo_id = GenerationId::parse(&format!("{lo}-ffffffff")).unwrap();
            let hi_id = GenerationId::parse(&format!("{hi}-00000000")).unwrap();
            assert!(lo_id < hi_id, "{lo_id:?} vs {hi_id:?}");
        }
    }

    #[test]
    fn parse_epoch_dir_round_trips() {
        for (epoch, salt) in [
            (0u64, (0u32, 0u32)),
            (42, SALT),
            (9_999_999_999, (u32::MAX, u32::MAX)),
        ] {
            let component = format!("{epoch:010}-{:08x}{:08x}", salt.0, salt.1);
            assert_eq!(
                parse_epoch_dir(&component),
                Some((epoch, salt)),
                "{component}"
            );
        }
    }

    #[test]
    fn parse_epoch_dir_rejects_malformed() {
        let malformed = [
            "",
            // Wrong widths
            "000000000-9d2f1c4a8b3e6f70",
            "00000000000-9d2f1c4a8b3e6f70",
            "0000000000-9d2f1c4a8b3e6f7",
            "0000000000-9d2f1c4a8b3e6f700",
            // Non-digit epoch, non-hex or uppercase salts
            "000000000a-9d2f1c4a8b3e6f70",
            "0000000000-9D2F1C4A8B3E6F70",
            "0000000000-9d2f1c4a8b3e6g70",
            // Missing separator; trailing slash
            "00000000009d2f1c4a8b3e6f70",
            "0000000000-9d2f1c4a8b3e6f70/",
        ];
        for component in malformed {
            assert_eq!(parse_epoch_dir(component), None, "{component:?}");
        }
    }

    #[test]
    fn parse_segment_key_round_trips() {
        let generation = generation();
        let segments = [
            SegmentKey {
                epoch: 0,
                salt: SALT,
                start: 0,
                end: 32,
            },
            SegmentKey {
                epoch: 7,
                salt: (0xa, 0xb),
                start: 32,
                end: 524_320,
            },
            SegmentKey {
                epoch: 9_999_999_999,
                salt: (u32::MAX, u32::MAX),
                start: u64::MAX - 1,
                end: u64::MAX,
            },
        ];
        for segment in segments {
            let key = segment_key(&generation, &segment);
            assert_eq!(
                parse_segment_key(&key),
                Some((generation.clone(), segment)),
                "{key}"
            );
        }
    }

    #[test]
    fn parse_segment_key_rejects_malformed() {
        let malformed = [
            "",
            // Wrong prefix or missing wal dir
            "generation/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal.zst",
            "generations/20260710T145900Z-3f8a2c1d/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal.zst",
            // Malformed generation id
            "generations/20260710T145900Z-3F8A2C1D/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal.zst",
            // Malformed epoch dir
            "generations/20260710T145900Z-3f8a2c1d/wal/000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal.zst",
            // Bad extension
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal",
            // Unpadded, non-numeric, or overflowing offsets
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/0-32.wal.zst",
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/000000000000000000zz-00000000000000000032.wal.zst",
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-99999999999999999999.wal.zst",
            // Empty or inverted ranges
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000032-00000000000000000032.wal.zst",
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000032-00000000000000000000.wal.zst",
            // Extra path components
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/x/00000000000000000000-00000000000000000032.wal.zst",
            "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000000032.wal.zst/x",
        ];
        for key in malformed {
            assert_eq!(parse_segment_key(key), None, "{key:?}");
        }
    }
}
