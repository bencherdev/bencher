use std::str::FromStr;

use bencher_json::JsonMember;
use bencher_valid::{Email, UserName};
use diesel::Queryable;
use uuid::Uuid;

use crate::{error::api_error, ApiError};

#[derive(Queryable)]
pub struct QueryMember {
    pub uuid: String,
    pub name: String,
    pub slug: String,
    pub email: String,
    pub role: String,
}

impl QueryMember {
    pub fn into_json(self) -> Result<JsonMember, ApiError> {
        let Self {
            uuid,
            name,
            slug,
            email,
            role,
        } = self;
        Ok(JsonMember {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            name: UserName::from_str(&name).map_err(api_error!())?,
            slug,
            email: Email::from_str(&email).map_err(api_error!())?,
            role: role.parse().map_err(ApiError::OrganizationRole)?,
        })
    }
}
