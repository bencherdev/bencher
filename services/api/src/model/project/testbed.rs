use std::str::FromStr;

use bencher_json::{JsonNewTestbed, JsonTestbed, NonEmpty, ResourceId, Slug};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use super::QueryProject;
use crate::{
    error::api_error,
    schema,
    schema::testbed as testbed_table,
    util::{query::fn_get_id, resource_id::fn_resource_id, slug::unwrap_child_slug},
    ApiError,
};

fn_resource_id!(testbed);

#[derive(Queryable)]
pub struct QueryTestbed {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
}

impl QueryTestbed {
    fn_get_id!(testbed);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::testbed::table
            .filter(schema::testbed::id.eq(id))
            .select(schema::testbed::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        project_id: i32,
        testbed: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::testbed::table
            .filter(schema::testbed::project_id.eq(project_id))
            .filter(resource_id(testbed)?)
            .first::<QueryTestbed>(conn)
            .map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonTestbed, ApiError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            ..
        } = self;
        Ok(JsonTestbed {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: NonEmpty::from_str(&name)?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = testbed_table)]
pub struct InsertTestbed {
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut SqliteConnection,
        project: &ResourceId,
        testbed: JsonNewTestbed,
    ) -> Result<Self, ApiError> {
        let project_id = QueryProject::from_resource_id(conn, project)?.id;
        Ok(Self::from_json_inner(conn, project_id, testbed))
    }

    pub fn localhost(conn: &mut SqliteConnection, project_id: i32) -> Self {
        Self::from_json_inner(conn, project_id, JsonNewTestbed::localhost())
    }

    pub fn from_json_inner(
        conn: &mut SqliteConnection,
        project_id: i32,
        testbed: JsonNewTestbed,
    ) -> Self {
        let JsonNewTestbed { name, slug } = testbed;
        let slug = unwrap_child_slug!(conn, project_id, name.as_ref(), slug, testbed, QueryTestbed);
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name: name.into(),
            slug,
        }
    }
}
