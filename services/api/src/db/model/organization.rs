use std::string::ToString;
use std::str::FromStr;

use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl, ExpressionMethods, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use super::user::InsertUser;
use crate::{
    db::schema::{self, organization as organization_table},
    util::http_error,
};

const ORGANIZATION_ERROR: &str = "Failed to create organization.";

#[derive(Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: String,
    pub name: String,
    pub slug: String,
}

impl InsertOrganization {
    pub fn from_user(
        conn: &mut SqliteConnection,
        insert_user: &InsertUser,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name: insert_user.name.clone(),
            slug: insert_user.slug.clone(),
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

impl QueryOrganization {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::organization::table
            .filter(schema::organization::uuid.eq(uuid.to_string()))
            .select(schema::organization::id)
            .first(conn)
            .map_err(|_| http_error!(ORGANIZATION_ERROR))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::organization::table
            .filter(schema::organization::id.eq(id))
            .select(schema::organization::uuid)
            .first(conn)
            .map_err(|_| http_error!(ORGANIZATION_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(ORGANIZATION_ERROR))
    }
}
