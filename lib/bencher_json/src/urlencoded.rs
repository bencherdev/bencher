use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

const COMMA: &str = "%2C";

#[derive(Debug, Error)]
pub enum UrlEncodedError {
    #[error("JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Serialize urlencoded: {0}")]
    Serialize(#[from] serde_urlencoded::ser::Error),
    #[error("Deserialize urlencoded: {0}")]
    Deserialize(#[from] serde_urlencoded::de::Error),
    #[error("UUID: {0}")]
    Uuid(#[from] uuid::Error),
}

pub fn comma_separated_list(list: &str) -> Result<Vec<Uuid>, UrlEncodedError> {
    let mut values = Vec::new();
    for value in list.split(',') {
        println!("{value}");
        values.push(value.parse()?);
    }
    Ok(values)
}

pub fn urlencoded_list<T>(values: &[T]) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    let mut list: Option<String> = None;
    for value in values {
        let element = urlencoded(value)?;
        if let Some(list) = list.as_mut() {
            list.push_str(COMMA);
            list.push_str(&element);
        } else {
            list = Some(element);
        }
    }
    Ok(list.unwrap_or_default())
}

fn urlencoded<T>(value: T) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    const KEY: &str = "_x";
    const KEY_EQUAL: &str = "_x=";
    Ok(serde_urlencoded::to_string([(KEY, value)])?
        .strip_prefix(KEY_EQUAL)
        .unwrap_or_default()
        .to_string())
}
