use std::str::FromStr;

use bencher_json::{
    JsonNewTestbed,
    JsonTestbed,
};
use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use super::project::QueryProject;
use crate::{
    db::{
        schema,
        schema::testbed as testbed_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

const TESTBED_ERROR: &str = "Failed to get testbed.";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
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
    pub fn get_id(conn: &SqliteConnection, uuid: Uuid) -> Result<i32, HttpError> {
        schema::testbed::table
            .filter(schema::testbed::uuid.eq(&uuid.to_string()))
            .select(schema::testbed::id)
            .first(conn)
            .map_err(|_| http_error!(TESTBED_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::testbed::table
            .filter(schema::testbed::id.eq(id))
            .select(schema::testbed::uuid)
            .first(conn)
            .map_err(|_| http_error!(TESTBED_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(TESTBED_ERROR))
    }

    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonTestbed, HttpError> {
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
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(TESTBED_ERROR))?,
            project_uuid: QueryProject::get_uuid(conn, project_id)?,
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
#[table_name = "testbed_table"]
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
    pub fn from_json(conn: &SqliteConnection, testbed: JsonNewTestbed) -> Result<Self, HttpError> {
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
        let slug = validate_slug(conn, &name, slug);
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

fn validate_slug(conn: &SqliteConnection, name: &str, slug: Option<String>) -> String {
    let mut slug = slug
        .map(|s| {
            if s == slug::slugify(&s) {
                s
            } else {
                slug::slugify(name)
            }
        })
        .unwrap_or_else(|| slug::slugify(name));

    if schema::testbed::table
        .filter(schema::testbed::slug.eq(&slug))
        .first::<QueryTestbed>(conn)
        .is_ok()
    {
        let rand_suffix = rand::random::<u32>().to_string();
        slug.push_str(&rand_suffix);
        slug
    } else {
        slug
    }
}
