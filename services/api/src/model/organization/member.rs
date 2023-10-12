use std::str::FromStr;

use bencher_json::{Email, JsonMember, Slug, UserName, UserUuid};

use crate::{util::to_date_time, ApiError};

#[derive(diesel::Queryable)]
pub struct QueryMember {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: String,
    pub role: String,
    pub created: i64,
    pub modified: i64,
}

impl QueryMember {
    pub fn into_json(self) -> Result<JsonMember, ApiError> {
        let Self {
            uuid,
            name,
            slug,
            email,
            role,
            created,
            modified,
        } = self;
        Ok(JsonMember {
            uuid,
            name,
            slug,
            email: Email::from_str(&email).map_err(ApiError::from)?,
            role: role.parse().map_err(ApiError::OrganizationRole)?,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }
}
