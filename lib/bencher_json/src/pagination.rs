#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DEFAULT_PER_PAGE: u8 = 8;

// TODO allow flattened, nested query params once possible
// https://github.com/oxidecomputer/dropshot/issues/721#issuecomment-1641027867
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPagination<S> {
    /// The field to sort by.
    /// If not specified, the default sort field is used.
    pub sort: Option<S>,
    /// The direction to sort by.
    /// If not specified, the default sort direction is used.
    pub direction: Option<JsonDirection>,
    /// The number of items to return per page.
    /// If not specified, the default number of items per page (8) is used.
    pub per_page: Option<u8>,
    /// The page number to return.
    /// If not specified, the first page is returned.
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonDirection {
    Asc,
    Desc,
}

impl<S> JsonPagination<S> {
    pub fn order(&self) -> S
    where
        S: Clone + Copy + Default,
    {
        self.sort.unwrap_or_default()
    }

    pub fn offset(&self) -> i64 {
        match self.page {
            Some(page @ 2u32..=u32::MAX) => i64::from((page - 1) * u32::from(self.per_page())),
            Some(0 | 1) | None => 0,
        }
    }

    pub fn limit(&self) -> i64 {
        i64::from(self.per_page())
    }

    fn per_page(&self) -> u8 {
        self.per_page.unwrap_or(DEFAULT_PER_PAGE)
    }
}
