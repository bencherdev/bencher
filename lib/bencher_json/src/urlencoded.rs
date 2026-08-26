use std::str::FromStr;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode, utf8_percent_encode};
use thiserror::Error;

// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT_SET: AsciiSet = CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
const FRAGMENT: &AsciiSet = &FRAGMENT_SET;

/// The fragment set, plus the two characters a list element must not spell literally.
///
/// A comma separates list elements, so an element that contains one has to escape it
/// or the list splits in the wrong place. A percent sign introduces an escape, so an
/// element that contains one has to escape it or the decode reads it as an escape it
/// never wrote. Neither matters for a UUID or a timestamp; both matter for a JSON
/// object.
const ELEMENT: &AsciiSet = &FRAGMENT_SET.add(b'%').add(b',');

#[derive(Debug, Error)]
pub enum UrlEncodedError {
    #[error("Empty `branches` parameter")]
    EmptyBranches,
    #[error("Empty `testbeds` parameter")]
    EmptyTestbeds,
    #[error("Empty `benchmarks` parameter")]
    EmptyBenchmarks,
    #[error("Empty `measures` parameter")]
    EmptyMeasures,
    #[error("Empty value in list: {0}")]
    EmptyValue(String),
    #[error("JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Serialize urlencoded: {0}")]
    Serialize(#[from] serde_urlencoded::ser::Error),
    #[error("Deserialize urlencoded: {0}")]
    Deserialize(#[from] serde_urlencoded::de::Error),
    #[error("UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Vec: {0:#?}")]
    Vec(Vec<(&'static str, Option<String>)>),
    #[error("urlencoded: {0}")]
    Urlencoded(String),
    #[error("Integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to convert milliseconds to timestamp: {0}")]
    Timestamp(i64),
    #[error("Failed to decode urlencoded: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub fn from_urlencoded_list<T>(list: &str) -> Result<Vec<T>, UrlEncodedError>
where
    T: FromStr,
{
    let mut values = Vec::new();
    for value in list.split(',') {
        if value.is_empty() {
            return Err(UrlEncodedError::EmptyValue(list.into()));
        }
        values.push(from_urlencoded(value)?);
    }
    Ok(values)
}

pub fn from_urlencoded_nullable_list<T>(
    list: Option<&str>,
) -> Result<Vec<Option<T>>, UrlEncodedError>
where
    T: FromStr,
{
    let Some(list) = list else {
        return Ok(Vec::new());
    };

    let mut values = Vec::new();
    for value in list.split(',') {
        values.push(if value.is_empty() {
            None
        } else {
            Some(from_urlencoded(value)?)
        });
    }
    Ok(values)
}

pub fn from_urlencoded<T>(input: &str) -> Result<T, UrlEncodedError>
where
    T: FromStr,
{
    let decoded = percent_decode(input.as_bytes());
    let decoded = decoded.decode_utf8()?;
    #[expect(
        clippy::map_err_ignore,
        reason = "original error type is not exposed by FromStr"
    )]
    decoded
        .parse()
        .map_err(|_| UrlEncodedError::Urlencoded(input.into()))
}

pub fn to_urlencoded_list<T>(values: &[T]) -> String
where
    T: ToString,
{
    let mut list = String::new();
    for value in values {
        let element = to_urlencoded(value);
        if list.is_empty() {
            list = element;
        } else {
            list.push(',');
            list.push_str(&element);
        }
    }
    list
}

pub fn to_urlencoded_optional_list<T>(values: &[Option<T>]) -> String
where
    T: ToString,
{
    let mut list = String::new();
    for value in values {
        let Some(value) = value else {
            list.push(',');
            continue;
        };

        let element = to_urlencoded(value);
        if list.is_empty() {
            list = element;
        } else {
            list.push(',');
            list.push_str(&element);
        }
    }
    list
}

pub fn to_urlencoded<T>(value: &T) -> String
where
    T: ToString,
{
    let value_str = value.to_string();
    let encoded = utf8_percent_encode(&value_str, FRAGMENT);
    encoded.collect()
}

/// A comma separated list whose elements may themselves contain commas.
///
/// Each element is encoded with the separator escaped, so the list splits back into
/// exactly the elements that went into it. [`from_urlencoded_list`] is the reader.
pub fn to_urlencoded_element_list<T>(values: &[T]) -> String
where
    T: ToString,
{
    values
        .iter()
        .map(to_urlencoded_element)
        .collect::<Vec<_>>()
        .join(",")
}

fn to_urlencoded_element<T>(value: &T) -> String
where
    T: ToString,
{
    let value_str = value.to_string();
    let encoded = utf8_percent_encode(&value_str, ELEMENT);
    encoded.collect()
}

#[cfg(test)]
mod tests {
    use super::{from_urlencoded_list, to_urlencoded_element_list};

    /// A list element that spells a comma round trips: the element is encoded with
    /// the separator escaped, so the split lands between elements and never inside
    /// one.
    #[test]
    fn element_list_round_trips_commas() {
        let elements = [
            r#"{"op":"read","size_mb":16}"#.to_owned(),
            r#"{"op":"write"}"#.to_owned(),
        ];

        let list = to_urlencoded_element_list(&elements);
        assert_eq!(
            list,
            "{%22op%22:%22read%22%2C%22size_mb%22:16},{%22op%22:%22write%22}"
        );
        assert_eq!(list.matches(',').count(), 1, "one separator, one comma");

        let decoded: Vec<String> =
            from_urlencoded_list(&list).expect("Failed to read the element list");
        assert_eq!(decoded, elements);
    }

    /// A percent sign inside an element is escaped too, so the decode never reads an
    /// escape the element did not write.
    #[test]
    fn element_list_round_trips_percents() {
        let elements = [r#"{"label":"100%2Cdone"}"#.to_owned()];

        let decoded: Vec<String> = from_urlencoded_list(&to_urlencoded_element_list(&elements))
            .expect("Failed to read the element list");

        assert_eq!(decoded, elements);
    }
}
