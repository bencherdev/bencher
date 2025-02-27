use std::fmt;

use bencher_schema::error::issue_error;
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct TotalCount(u32);

impl TryFrom<i64> for TotalCount {
    type Error = HttpError;

    fn try_from(total_count: i64) -> Result<Self, Self::Error> {
        match u32::try_from(total_count) {
            Ok(total_count) => Ok(TotalCount(total_count)),
            Err(err) => Err(issue_error(
                "Failed to count resource total.",
                &format!("Failed to count resource total: {total_count}"),
                err,
            )),
        }
    }
}

impl fmt::Display for TotalCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TotalCount {
    pub const ZERO: Self = TotalCount(0);
    pub const ONE: Self = TotalCount(1);
}
