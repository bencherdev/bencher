use std::string::ToString;

use diesel::{Insertable, Queryable, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use super::user::InsertUser;
use crate::db::schema::organization as organization_table;

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
