#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// https://stackoverflow.com/questions/44331037/how-can-i-distinguish-between-a-deserialized-field-that-is-missing-and-one-that
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonMaybe<T> {
    #[default]
    Undefined,
    Null,
    Value(T),
}

// #[allow(dead_code)]
impl<T> JsonMaybe<T> {
    pub fn is_undefined(&self) -> bool {
        matches!(self, JsonMaybe::Undefined)
    }
}

impl<T> From<Option<T>> for JsonMaybe<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => JsonMaybe::Value(v),
            None => JsonMaybe::Null,
        }
    }
}

impl<T, U> From<JsonMaybe<T>> for Option<Option<U>>
where
    U: From<T>,
{
    fn from(maybe: JsonMaybe<T>) -> Self {
        match maybe {
            JsonMaybe::Undefined => None,
            JsonMaybe::Null => Some(None),
            JsonMaybe::Value(v) => Some(Some(v.into())),
        }
    }
}

impl<T: Serialize> Serialize for JsonMaybe<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            // should have been skipped
            JsonMaybe::Undefined => Err(serde::ser::Error::custom(
                r#"JsonMaybe fields need to be annotated with: #[serde(default, skip_serializing_if = "JsonMaybe::is_undefined")]"#,
            )),
            JsonMaybe::Null => serializer.serialize_none(),
            JsonMaybe::Value(v) => v.serialize(serializer),
        }
    }
}

impl<'de, T> Deserialize<'de> for JsonMaybe<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::deserialize(deserializer).map(Into::into)
    }
}
