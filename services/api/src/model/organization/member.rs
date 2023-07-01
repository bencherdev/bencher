use std::str::FromStr;

use bencher_json::{Email, JsonMember, Slug, UserName};
use diesel::Queryable;
use uuid::Uuid;

use crate::{error::api_error, util::to_date_time, ApiError};

#[derive(Queryable)]
pub struct QueryMember {
    pub uuid: String,
    pub name: String,
    pub slug: String,
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
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            name: UserName::from_str(&name).map_err(api_error!())?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
            email: Email::from_str(&email).map_err(api_error!())?,
            role: role.parse().map_err(ApiError::OrganizationRole)?,
            created: to_date_time(created).map_err(api_error!())?,
            modified: to_date_time(modified).map_err(api_error!())?,
        })
    }
}
