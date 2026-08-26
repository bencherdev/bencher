//! `SQLite`'s [JSONB][jsonb] binary encoding.
//!
//! Two writers reach the `parameter.set` column: this encoder, and
//! `SQLite`'s own `jsonb()` in the migration that mints the empty parameter set.
//! `UNIQUE(benchmark_id, "set")` compares bytes, so the two have to agree
//! byte for byte, and `SQLite` is the definition of correct. Where the format
//! admits more than one encoding of the same value, this matches what `jsonb()`
//! produces over the RFC 8785 (JCS) canonical text of the same parameter set.
//!
//! An element is a header followed by a payload. The header's low nibble is the
//! element type. Its high nibble is the payload size when the size is at most
//! 11, and otherwise selects a big endian size that follows the header, in the
//! most compact of the one, two, and four byte forms.
//!
//! [jsonb]: https://sqlite.org/jsonb.html

use super::{write_json_string, write_json_string_body};

/// `null`.
const NULL: u8 = 0x00;
/// `true`.
const TRUE: u8 = 0x01;
/// `false`.
const FALSE: u8 = 0x02;
/// An integer, as JSON spells it.
const INT: u8 = 0x03;
/// An integer in a JSON5 notation, which canonical JSON never produces.
const INT5: u8 = 0x04;
/// A float, as JSON spells it.
const FLOAT: u8 = 0x05;
/// A float in a JSON5 notation, which canonical JSON never produces.
const FLOAT5: u8 = 0x06;
/// A string whose source text holds no escapes.
const TEXT: u8 = 0x07;
/// A string whose source text holds JSON escapes, carried through verbatim.
const TEXTJ: u8 = 0x08;
/// A string holding JSON5 escapes, which canonical JSON never produces.
const TEXT5: u8 = 0x09;
/// SQL text that has to be escaped to become JSON.
const TEXTRAW: u8 = 0x0a;
/// An array.
const ARRAY: u8 = 0x0b;
/// An object.
const OBJECT: u8 = 0x0c;

/// The largest payload size a header can carry on its own.
const INLINE_SIZE: u32 = 11;
/// The high nibble marking a one byte payload size.
const ONE_BYTE_SIZE: u8 = 0xc0;
/// The high nibble marking a two byte payload size.
const TWO_BYTE_SIZE: u8 = 0xd0;
/// The high nibble marking a four byte payload size.
const FOUR_BYTE_SIZE: u8 = 0xe0;

/// How deep a blob may nest before it is treated as malformed.
///
/// A parameter set is an object of scalars, so anything deeper than that is
/// already corruption. The limit is what keeps the decoder off the stack.
const MAX_DEPTH: u8 = 32;

/// A JSONB object under construction.
///
/// Members are appended in the order they are given, which for a parameter set
/// is the RFC 8785 key order. The encoder never sorts.
#[derive(Debug, Default)]
pub struct Object(Vec<u8>);

impl Object {
    /// Append a `null` member.
    pub fn insert_null(&mut self, key: &str) -> Result<(), JsonbError> {
        self.insert(key, NULL, &[])
    }

    /// Append a boolean member.
    pub fn insert_bool(&mut self, key: &str, value: bool) -> Result<(), JsonbError> {
        self.insert(key, if value { TRUE } else { FALSE }, &[])
    }

    /// Append a number member.
    ///
    /// The payload is the canonical number text, written through unchanged.
    /// `SQLite`'s parser reads a number as a float when its text holds a fraction
    /// or an exponent and as an integer otherwise, whatever its magnitude, so an
    /// integer above `i64` is still an integer here.
    pub fn insert_number(&mut self, key: &str, canonical: &str) -> Result<(), JsonbError> {
        let element = if canonical.contains(['.', 'e', 'E']) {
            FLOAT
        } else {
            INT
        };
        self.insert(key, element, canonical.as_bytes())
    }

    /// Append a string member.
    pub fn insert_string(&mut self, key: &str, value: &str) -> Result<(), JsonbError> {
        let (element, payload) = string_element(value);
        self.insert(key, element, payload.as_bytes())
    }

    fn insert(&mut self, key: &str, element: u8, payload: &[u8]) -> Result<(), JsonbError> {
        let (key_element, key_payload) = string_element(key);
        push_element(&mut self.0, key_element, key_payload.as_bytes())?;
        push_element(&mut self.0, element, payload)
    }

    /// The JSONB encoding of the object.
    pub fn into_blob(self) -> Result<Vec<u8>, JsonbError> {
        let mut blob = Vec::with_capacity(self.0.len().saturating_add(5));
        push_element(&mut blob, OBJECT, &self.0)?;
        Ok(blob)
    }
}

/// A JSONB array under construction.
///
/// Elements are appended in the order they are given, which for a parameters
/// filter is the order the canonical form puts its sets in. The encoder never
/// sorts.
#[derive(Debug, Default)]
pub struct Array(Vec<u8>);

impl Array {
    /// Append one element, given its own JSONB encoding.
    ///
    /// An element of an array is encoded exactly as it is on its own, so what
    /// [`Object::into_blob`] returns is what goes here.
    pub fn push(&mut self, element: &[u8]) {
        self.0.extend_from_slice(element);
    }

    /// The JSONB encoding of the array.
    pub fn into_blob(self) -> Result<Vec<u8>, JsonbError> {
        let mut blob = Vec::with_capacity(self.0.len().saturating_add(5));
        push_element(&mut blob, ARRAY, &self.0)?;
        Ok(blob)
    }
}

/// A string's element type and the payload that goes with it.
///
/// The payload is the string as it appears between the quotes of the canonical
/// text, so a string that needs no escape is carried literally and a string that
/// does is carried escaped. `SQLite` reads a string as `TEXT` until it meets a
/// backslash, at which point the element becomes `TEXTJ`.
fn string_element(string: &str) -> (u8, String) {
    let mut payload = String::new();
    write_json_string_body(string, &mut payload);
    let element = if payload.contains('\\') { TEXTJ } else { TEXT };
    (element, payload)
}

/// Append one element: a header, then its payload.
fn push_element(blob: &mut Vec<u8>, element: u8, payload: &[u8]) -> Result<(), JsonbError> {
    // SQLite holds payload sizes in a `u32`, so anything larger has no encoding
    // to agree with rather than one this could guess at.
    let size = u32::try_from(payload.len()).map_err(JsonbError::PayloadSize)?;
    if size <= INLINE_SIZE {
        blob.push(element | size_nibble(size));
    } else if u8::try_from(size).is_ok() {
        blob.push(element | ONE_BYTE_SIZE);
        blob.push(size_byte(size, 0));
    } else if u16::try_from(size).is_ok() {
        blob.push(element | TWO_BYTE_SIZE);
        blob.push(size_byte(size, 8));
        blob.push(size_byte(size, 0));
    } else {
        blob.push(element | FOUR_BYTE_SIZE);
        blob.push(size_byte(size, 24));
        blob.push(size_byte(size, 16));
        blob.push(size_byte(size, 8));
        blob.push(size_byte(size, 0));
    }
    blob.extend_from_slice(payload);
    Ok(())
}

/// A payload size small enough to ride in the header's high nibble.
///
/// The caller only reaches this with a size at most the inline maximum, so the
/// mask takes the whole of it. Masking first is what puts the shift in the `u8`
/// domain, where a nibble cannot outgrow the byte it moves into.
const fn size_nibble(size: u32) -> u8 {
    ((size & 0x0f) as u8) << 4
}

/// One byte of a big endian payload size.
const fn size_byte(size: u32, shift: u32) -> u8 {
    ((size >> shift) & 0xff) as u8
}

/// Render a JSONB blob as JSON text.
///
/// Both writers land in the same column, so this reads back what this encoder
/// wrote and what `SQLite`'s `jsonb()` wrote.
pub fn to_json(blob: &[u8]) -> Result<String, JsonbError> {
    let mut json = String::new();
    let index = write_value(blob, 0, 0, &mut json)?;
    if index == blob.len() {
        Ok(json)
    } else {
        Err(JsonbError::TrailingBytes)
    }
}

/// Write the element at `index` as JSON text and return the index just past it.
fn write_value(
    blob: &[u8],
    index: usize,
    depth: u8,
    json: &mut String,
) -> Result<usize, JsonbError> {
    if depth > MAX_DEPTH {
        return Err(JsonbError::TooDeep);
    }
    let header = *blob.get(index).ok_or(JsonbError::Truncated)?;
    let (size, header_size) = payload_size(blob, index, header >> 4)?;
    let start = index
        .checked_add(header_size)
        .ok_or(JsonbError::Truncated)?;
    let end = start.checked_add(size).ok_or(JsonbError::Truncated)?;
    let payload = blob.get(start..end).ok_or(JsonbError::Truncated)?;

    match header & 0x0f {
        NULL => json.push_str("null"),
        TRUE => json.push_str("true"),
        FALSE => json.push_str("false"),
        // The payload is the number as JSON spells it, so it needs no parsing
        // step here: the magnitude is the caller's problem, not the codec's.
        INT | FLOAT => json.push_str(text(payload)?),
        TEXT | TEXTRAW => write_json_string(text(payload)?, json),
        TEXTJ => {
            json.push('"');
            json.push_str(text(payload)?);
            json.push('"');
        },
        ARRAY => write_array(blob, start, end, depth, json)?,
        OBJECT => write_object(blob, start, end, depth, json)?,
        element @ (INT5 | FLOAT5 | TEXT5) => return Err(JsonbError::Json5(element)),
        element => return Err(JsonbError::Element(element)),
    }
    Ok(end)
}

fn write_array(
    blob: &[u8],
    start: usize,
    end: usize,
    depth: u8,
    json: &mut String,
) -> Result<(), JsonbError> {
    json.push('[');
    let mut index = start;
    while index < end {
        if index > start {
            json.push(',');
        }
        index = write_value(blob, index, depth.saturating_add(1), json)?;
    }
    if index == end {
        json.push(']');
        Ok(())
    } else {
        Err(JsonbError::Truncated)
    }
}

fn write_object(
    blob: &[u8],
    start: usize,
    end: usize,
    depth: u8,
    json: &mut String,
) -> Result<(), JsonbError> {
    json.push('{');
    let mut index = start;
    while index < end {
        if index > start {
            json.push(',');
        }
        index = write_value(blob, index, depth.saturating_add(1), json)?;
        if index >= end {
            return Err(JsonbError::Truncated);
        }
        json.push(':');
        index = write_value(blob, index, depth.saturating_add(1), json)?;
    }
    if index == end {
        json.push('}');
        Ok(())
    } else {
        Err(JsonbError::Truncated)
    }
}

/// The payload size an element header describes, and the size of that header.
fn payload_size(blob: &[u8], index: usize, marker: u8) -> Result<(usize, usize), JsonbError> {
    let size_bytes = match marker {
        0..=11 => return Ok((usize::from(marker), 1)),
        12 => 1,
        13 => 2,
        14 => 4,
        // 15, the eight byte form, which SQLite's own encoder never writes.
        _ => 8,
    };
    let start = index.checked_add(1).ok_or(JsonbError::Truncated)?;
    let mut size = 0usize;
    for offset in 0..size_bytes {
        let byte = *blob
            .get(start.checked_add(offset).ok_or(JsonbError::Truncated)?)
            .ok_or(JsonbError::Truncated)?;
        size = size
            .checked_mul(0x100)
            .and_then(|size| size.checked_add(usize::from(byte)))
            .ok_or(JsonbError::PayloadTooLarge)?;
    }
    let header_size = size_bytes.checked_add(1).ok_or(JsonbError::Truncated)?;
    Ok((size, header_size))
}

fn text(payload: &[u8]) -> Result<&str, JsonbError> {
    std::str::from_utf8(payload).map_err(JsonbError::Utf8)
}

#[derive(Debug, thiserror::Error)]
pub enum JsonbError {
    #[error("Failed to encode a JSONB payload: {0}")]
    PayloadSize(std::num::TryFromIntError),
    #[error("JSONB payload is larger than this platform can address")]
    PayloadTooLarge,
    #[error("JSONB blob ends inside an element")]
    Truncated,
    #[error("JSONB blob has bytes after its value")]
    TrailingBytes,
    #[error("JSONB blob nests deeper than a JSON value can")]
    TooDeep,
    #[error("JSONB element ({0:#04x}) is JSON5, which is not JSON")]
    Json5(u8),
    #[error("JSONB element ({0:#04x}) is not a JSON value")]
    Element(u8),
    #[error("JSONB payload is not UTF-8: {0}")]
    Utf8(std::str::Utf8Error),
}

#[cfg(test)]
mod tests {
    use super::{JsonbError, Object, to_json};

    fn object() -> Object {
        Object::default()
    }

    #[test]
    fn empty_object_is_one_byte() {
        let blob = object().into_blob().expect("Failed to encode");
        assert_eq!(blob, vec![0x0c], "the empty object is a single header byte");
        assert_eq!(to_json(&blob).expect("Failed to decode"), "{}");
    }

    #[test]
    fn scalars_round_trip() {
        let mut object = object();
        object.insert_bool("debug", true).expect("Failed to encode");
        object.insert_null("gap").expect("Failed to encode");
        object
            .insert_number("threads", "4")
            .expect("Failed to encode");
        object
            .insert_number("tolerance", "1e-7")
            .expect("Failed to encode");
        object
            .insert_string("path", "C:\\bench\\x")
            .expect("Failed to encode");
        object
            .insert_string("os", "linux")
            .expect("Failed to encode");
        let blob = object.into_blob().expect("Failed to encode");

        assert_eq!(
            to_json(&blob).expect("Failed to decode"),
            r#"{"debug":true,"gap":null,"threads":4,"tolerance":1e-7,"path":"C:\\bench\\x","os":"linux"}"#,
            "every scalar survives a round trip in the order it was inserted"
        );
    }

    #[test]
    fn payload_larger_than_the_inline_size() {
        // A payload of 12 bytes or more moves the size out of the header nibble.
        let mut object = object();
        object
            .insert_string("k", &"x".repeat(300))
            .expect("Failed to encode");
        let blob = object.into_blob().expect("Failed to encode");
        assert_eq!(
            to_json(&blob).expect("Failed to decode"),
            format!(r#"{{"k":"{}"}}"#, "x".repeat(300)),
            "a payload past the inline size still round trips"
        );
    }

    #[test]
    fn malformed_blobs_are_rejected() {
        assert!(
            matches!(to_json(&[]), Err(JsonbError::Truncated)),
            "an empty blob has no value in it"
        );
        // An object header claiming three payload bytes that are not there.
        assert!(
            matches!(to_json(&[0x3c]), Err(JsonbError::Truncated)),
            "a header without its payload is truncated"
        );
        // A valid `{}` followed by a byte that belongs to no element.
        assert!(
            matches!(to_json(&[0x0c, 0x0c]), Err(JsonbError::TrailingBytes)),
            "bytes after the value are not part of it"
        );
        // JSON5 elements, which canonical JSON never produces.
        assert!(
            matches!(to_json(&[0x14, 0x30]), Err(JsonbError::Json5(0x04))),
            "JSON5 is not JSON"
        );
        // An object whose payload ends between a key and its value.
        assert!(
            matches!(to_json(&[0x2c, 0x17, 0x78]), Err(JsonbError::Truncated)),
            "an object member without its value is truncated"
        );
    }
}
