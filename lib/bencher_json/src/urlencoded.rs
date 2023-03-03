use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

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
    #[error("URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Vec: {0:#?}")]
    Vec(Vec<(&'static str, Option<String>)>),
    #[error("urlencoded: {0}")]
    Urlencoded(String),
}

pub fn from_urlencoded_list<T>(list: &str) -> Result<Vec<T>, UrlEncodedError>
where
    T: DeserializeOwned,
{
    let mut values = Vec::new();
    for value in list.split(',') {
        values.push(from_urlencoded(value)?);
    }
    Ok(values)
}

pub fn from_urlencoded<T>(input: &str) -> Result<T, UrlEncodedError>
where
    T: DeserializeOwned,
{
    let urlencoded = format!("{input}=");
    Ok(serde_urlencoded::from_str::<Vec<(T, String)>>(&urlencoded)?
        .pop()
        .ok_or_else(|| UrlEncodedError::Urlencoded(input.into()))?
        .0)
}

pub fn to_urlencoded_list<T>(values: &[T]) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    let mut list: Option<String> = None;
    for value in values {
        let element = to_urlencoded(value)?;
        if let Some(list) = list.as_mut() {
            list.push(',');
            list.push_str(&element);
        } else {
            list = Some(element);
        }
    }
    Ok(list.unwrap_or_default())
}

pub fn to_urlencoded<T>(value: &T) -> Result<String, UrlEncodedError>
where
    T: Serialize,
{
    Ok(serde_urlencoded::to_string([(value, "")])?
        .strip_suffix('=')
        .unwrap_or_default()
        .to_string())
}
