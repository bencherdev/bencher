use chrono::{DateTime, TimeZone, Utc};

use crate::ApiError;

pub mod cors;
pub mod error;
pub mod headers;
pub mod query;
pub mod resource_id;
pub mod same_project;
pub mod slug;

pub fn map_u32(signed: Option<i64>) -> Result<Option<u32>, std::num::TryFromIntError> {
    Ok(if let Some(signed) = signed {
        Some(signed.try_into()?)
    } else {
        None
    })
}

// https://docs.rs/chrono/latest/chrono/serde/ts_seconds/index.html
pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, ApiError> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .ok_or(ApiError::Timestamp(timestamp))
}
