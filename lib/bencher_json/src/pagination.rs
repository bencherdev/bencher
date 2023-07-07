#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPagination<T> {
    pub sort: Option<T>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonDirection {
    Asc,
    Desc,
}

impl<T> JsonPagination<T> {
    pub fn order(&self) -> T
    where
        T: Clone + Copy + Default,
    {
        self.sort.unwrap_or_default()
    }

    pub fn offset(&self) -> i64 {
        match self.page {
            Some(page @ 2_u32..=u32::MAX) => i64::from((page - 1) * u32::from(self.per_page())),
            Some(0 | 1) | None => 0,
        }
    }

    pub fn limit(&self) -> i64 {
        i64::from(self.per_page())
    }

    fn per_page(&self) -> u8 {
        self.per_page.unwrap_or(8)
    }
}
