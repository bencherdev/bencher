use std::str::FromStr;

use percent_encoding::{percent_decode, utf8_percent_encode, AsciiSet, CONTROLS};
use thiserror::Error;

// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

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

pub fn from_urlencoded<T>(input: &str) -> Result<T, UrlEncodedError>
where
    T: FromStr,
{
    let decoded = percent_decode(input.as_bytes());
    let decoded = decoded.decode_utf8()?;
    #[allow(clippy::map_err_ignore)]
    decoded
        .parse()
        .map_err(|_| UrlEncodedError::Urlencoded(input.into()))
}

pub fn to_urlencoded_list<T>(values: &[T]) -> String
where
    T: ToString,
{
    let mut list: Option<String> = None;
    for value in values {
        let element = to_urlencoded(value);
        if let Some(list) = list.as_mut() {
            list.push(',');
            list.push_str(&element);
        } else {
            list = Some(element);
        }
    }
    list.unwrap_or_default()
}

pub fn to_urlencoded<T>(value: &T) -> String
where
    T: ToString,
{
    let value_str = value.to_string();
    let encoded = utf8_percent_encode(&value_str, FRAGMENT);
    encoded.collect()
}
