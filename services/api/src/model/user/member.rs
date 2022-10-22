use std::str::FromStr;

use bencher_json::{member::JsonOrganizationRole, JsonMember, JsonSignup, JsonUser, ResourceId};
use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use email_address_parser::EmailAddress;
use uuid::Uuid;

use crate::{
    diesel::ExpressionMethods,
    error::api_error,
    schema::{self, user as user_table},
    util::{resource_id::fn_resource_id, slug::unwrap_slug},
    ApiError,
};

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
