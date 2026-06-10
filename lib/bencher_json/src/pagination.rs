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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{DEFAULT_PER_PAGE, JsonDirection, JsonPagination};

    const DEFAULT_LIMIT: i64 = 8;
    /// `(u32::MAX - 1) * 1 = 4_294_967_294`
    const MAX_PAGE_PER_PAGE_ONE_OFFSET: i64 = 0xFFFF_FFFE;
    /// The largest page that does not overflow `u32` with the default `per_page` of 8:
    /// `536_870_912`, since `(536_870_912 - 1) * 8 = 4_294_967_288 <= u32::MAX`
    const MAX_PAGE_DEFAULT_PER_PAGE: u32 = 0x2000_0000;
    /// `(536_870_912 - 1) * 8 = 4_294_967_288`
    const MAX_PAGE_DEFAULT_PER_PAGE_OFFSET: i64 = 0xFFFF_FFF8;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum TestSort {
        #[default]
        Name,
        Created,
    }

    fn pagination(per_page: Option<u8>, page: Option<u32>) -> JsonPagination<TestSort> {
        JsonPagination {
            sort: None,
            direction: None,
            per_page,
            page,
        }
    }

    #[test]
    fn offset_page_none() {
        assert_eq!(pagination(None, None).offset(), 0);
    }

    #[test]
    fn offset_page_zero() {
        assert_eq!(pagination(None, Some(0)).offset(), 0);
    }

    #[test]
    fn offset_page_one() {
        assert_eq!(pagination(None, Some(1)).offset(), 0);
    }

    #[test]
    fn offset_page_two_default_per_page() {
        assert_eq!(pagination(None, Some(2)).offset(), DEFAULT_LIMIT);
    }

    #[test]
    fn offset_page_three_default_per_page() {
        assert_eq!(pagination(None, Some(3)).offset(), 2 * DEFAULT_LIMIT);
    }

    #[test]
    fn offset_page_two_custom_per_page() {
        assert_eq!(pagination(Some(8), Some(2)).offset(), 8);
        assert_eq!(pagination(Some(25), Some(2)).offset(), 25);
        assert_eq!(pagination(Some(25), Some(4)).offset(), 75);
    }

    #[test]
    fn offset_per_page_max() {
        assert_eq!(
            pagination(Some(u8::MAX), Some(2)).offset(),
            i64::from(u8::MAX)
        );
    }

    // A `per_page` of zero is representable and yields an offset of zero
    // for any page, along with a limit of zero.
    #[test]
    fn offset_per_page_zero() {
        assert_eq!(pagination(Some(0), Some(5)).offset(), 0);
        assert_eq!(pagination(Some(0), Some(5)).limit(), 0);
    }

    #[test]
    fn offset_max_page_per_page_one() {
        assert_eq!(
            pagination(Some(1), Some(u32::MAX)).offset(),
            MAX_PAGE_PER_PAGE_ONE_OFFSET
        );
    }

    #[test]
    fn offset_max_page_default_per_page() {
        assert_eq!(
            pagination(None, Some(MAX_PAGE_DEFAULT_PER_PAGE)).offset(),
            MAX_PAGE_DEFAULT_PER_PAGE_OFFSET
        );
    }

    // The offset multiplication `(page - 1) * per_page` is performed in `u32`,
    // so any product above `u32::MAX` overflows: it panics in debug builds
    // and silently wraps to a much smaller offset in release builds.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "attempt to multiply with overflow")]
    fn offset_overflow_panics_in_debug() {
        let _offset = pagination(Some(2), Some(u32::MAX)).offset();
    }

    #[test]
    fn limit_default() {
        assert_eq!(DEFAULT_LIMIT, i64::from(DEFAULT_PER_PAGE));
        assert_eq!(pagination(None, None).limit(), DEFAULT_LIMIT);
    }

    #[test]
    fn limit_custom() {
        assert_eq!(pagination(Some(1), None).limit(), 1);
        assert_eq!(pagination(Some(100), None).limit(), 100);
        assert_eq!(pagination(Some(u8::MAX), None).limit(), i64::from(u8::MAX));
    }

    #[test]
    fn order_default_when_sort_none() {
        assert_eq!(pagination(None, None).order(), TestSort::Name);
    }

    #[test]
    fn order_uses_explicit_sort() {
        let json_pagination = JsonPagination {
            sort: Some(TestSort::Created),
            direction: None,
            per_page: None,
            page: None,
        };
        assert_eq!(json_pagination.order(), TestSort::Created);
    }

    #[test]
    fn direction_serde() {
        assert_eq!(
            serde_json::to_string(&JsonDirection::Asc).unwrap(),
            r#""asc""#
        );
        assert_eq!(
            serde_json::to_string(&JsonDirection::Desc).unwrap(),
            r#""desc""#
        );
        let direction: JsonDirection = serde_json::from_str(r#""desc""#).unwrap();
        assert!(matches!(direction, JsonDirection::Desc));
    }

    #[test]
    fn query_string_deserialization() {
        let json_pagination: JsonPagination<TestSort> =
            serde_urlencoded::from_str("sort=created&direction=desc&per_page=2&page=3").unwrap();
        assert_eq!(json_pagination.sort, Some(TestSort::Created));
        assert!(matches!(
            json_pagination.direction,
            Some(JsonDirection::Desc)
        ));
        assert_eq!(json_pagination.per_page, Some(2));
        assert_eq!(json_pagination.page, Some(3));
        assert_eq!(json_pagination.offset(), 4);
        assert_eq!(json_pagination.limit(), 2);
    }

    #[test]
    fn query_string_deserialization_empty() {
        let json_pagination: JsonPagination<TestSort> = serde_urlencoded::from_str("").unwrap();
        assert_eq!(json_pagination.sort, None);
        assert!(json_pagination.direction.is_none());
        assert_eq!(json_pagination.per_page, None);
        assert_eq!(json_pagination.page, None);
        assert_eq!(json_pagination.offset(), 0);
        assert_eq!(json_pagination.limit(), DEFAULT_LIMIT);
    }
}
