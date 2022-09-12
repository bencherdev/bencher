use std::str::FromStr;

use bencher_json::{JsonNewTestbed, JsonTestbed};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use super::project::QueryProject;
use crate::{
    schema,
    schema::testbed as testbed_table,
    util::{map_http_error, slug::unwrap_slug},
};

#[derive(Queryable)]
pub struct QueryTestbed {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
}

impl QueryTestbed {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::testbed::table
            .filter(schema::testbed::uuid.eq(uuid.to_string()))
            .select(schema::testbed::id)
            .first(conn)
            .map_err(map_http_error!("Failed to get testbed."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::testbed::table
            .filter(schema::testbed::id.eq(id))
            .select(schema::testbed::uuid)
            .first(conn)
            .map_err(map_http_error!("Failed to get testbed."))?;
        Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get testbed."))
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonTestbed, HttpError> {
        let Self {
            id: _,
            uuid,
            project_id,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
        } = self;
        Ok(JsonTestbed {
            uuid: Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get testbed."))?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
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
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut SqliteConnection,
        testbed: JsonNewTestbed,
    ) -> Result<Self, HttpError> {
        let JsonNewTestbed {
            project,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
        } = testbed;
        let slug = unwrap_slug!(conn, &name, slug, testbed, QueryTestbed);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            project_id: QueryProject::from_resource_id(conn, &project)?.id,
            name,
            slug,
            os_name,
            os_version,
            runtime_name,
            runtime_version,
            cpu,
            ram,
            disk,
        })
    }
}
