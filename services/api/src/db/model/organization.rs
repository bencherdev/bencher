use std::{str::FromStr, string::ToString};

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use diesel::{
    expression_methods::BoolExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use dropshot::{HttpError, RequestContext};
use url::Url;
use uuid::Uuid;

use super::user::QueryUser;
use crate::{
    db::schema::{self, organization as organization_table},
    diesel::ExpressionMethods,
    util::{http_error, Context},
};

const PROJECT_ERROR: &str = "Failed to get project.";

#[derive(Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: String,
    pub name: String,
    pub slug: String,
}

impl InsertOrganization {
    pub fn for_user(
        conn: &mut SqliteConnection,
        query_user: &QueryUser,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name: query_user.name.clone(),
            slug: query_user.slug.clone(),
        })
    }
}

#[derive(Queryable)]
pub struct QueryOrganization {
    pub id: i32,
    pub uuid: String,
    pub name: String,
    pub slug: String,
}
