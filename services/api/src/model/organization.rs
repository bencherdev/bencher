use std::str::FromStr;
use std::string::ToString;

use bencher_json::{JsonNewOrganization, JsonOrganization, ResourceId};
use diesel::{
    BoolExpressionMethods, ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::user::InsertUser;
use crate::{
    schema::{self, organization as organization_table},
    util::{map_http_error, slug::unwrap_slug},
};

#[derive(Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: String,
    pub name: String,
    pub slug: String,
}

impl InsertOrganization {
    pub fn from_json(
        conn: &mut SqliteConnection,
        organization: JsonNewOrganization,
    ) -> Result<Self, HttpError> {
        let JsonNewOrganization { name, slug } = organization;
        let slug = unwrap_slug!(conn, &name, slug, organization, QueryOrganization);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name,
            slug,
        })
    }

    pub fn from_user(insert_user: &InsertUser) -> Result<Self, HttpError> {
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
            .map_err(map_http_error!("Failed to create organization."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::organization::table
            .filter(schema::organization::id.eq(id))
            .select(schema::organization::uuid)
            .first(conn)
            .map_err(map_http_error!("Failed to create organization."))?;
        Uuid::from_str(&uuid).map_err(map_http_error!("Failed to create organization."))
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        organization: &ResourceId,
    ) -> Result<Self, HttpError> {
        let org = &organization.0;
        schema::organization::table
            .filter(
                schema::organization::slug
                    .eq(org)
                    .or(schema::organization::uuid.eq(org)),
            )
            .first::<QueryOrganization>(conn)
            .map_err(map_http_error!("Failed to create organization."))
    }

    pub fn into_json(self) -> Result<JsonOrganization, HttpError> {
        let Self {
            id: _,
            uuid,
            name,
            slug,
        } = self;
        Ok(JsonOrganization {
            uuid: Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get organization."))?,
            name,
            slug,
        })
    }
}
