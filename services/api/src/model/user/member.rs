use std::str::FromStr;

use bencher_json::{member::JsonOrganizationRole, JsonMember};
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
            name,
            slug,
            email,
            role: JsonOrganizationRole::from_str(&role)
                .map_err(|e| ApiError::OrganizationRole(e))?,
        })
    }
}
