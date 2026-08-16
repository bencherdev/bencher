//! SQLite's JSONB binary encoding.
//!
//! Two writers reach the `parameter.parameters` column: this encoder, and
//! SQLite's own `jsonb()` in the migration that mints the empty parameter set.
//! `UNIQUE(benchmark_id, parameters)` compares bytes, so the two have to agree
//! byte for byte, and SQLite is the definition of correct.

/// A JSONB object under construction.
#[derive(Debug, Default)]
pub struct Object(Vec<u8>);

impl Object {
    /// Append a `null` member.
    pub fn insert_null(&mut self, _key: &str) -> Result<(), JsonbError> {
        Err(JsonbError::Unimplemented)
    }

    /// Append a boolean member.
    pub fn insert_bool(&mut self, _key: &str, _value: bool) -> Result<(), JsonbError> {
        Err(JsonbError::Unimplemented)
    }

    /// Append a number member, whose payload is the canonical number text.
    pub fn insert_number(&mut self, _key: &str, _canonical: &str) -> Result<(), JsonbError> {
        Err(JsonbError::Unimplemented)
    }

    /// Append a string member.
    pub fn insert_string(&mut self, _key: &str, _value: &str) -> Result<(), JsonbError> {
        Err(JsonbError::Unimplemented)
    }

    /// The JSONB encoding of the object.
    pub fn into_blob(self) -> Result<Vec<u8>, JsonbError> {
        Err(JsonbError::Unimplemented)
    }
}

/// Render a JSONB blob as JSON text.
pub fn to_json(_blob: &[u8]) -> Result<String, JsonbError> {
    Err(JsonbError::Unimplemented)
}

#[derive(Debug, thiserror::Error)]
pub enum JsonbError {
    #[error("The JSONB codec is not implemented yet")]
    Unimplemented,
}
