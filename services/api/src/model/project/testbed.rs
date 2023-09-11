use std::str::FromStr;

use bencher_json::{
    project::testbed::{JsonUpdateTestbed, TESTBED_LOCALHOST_STR},
    JsonNewTestbed, JsonTestbed, NonEmpty, ResourceId, Slug,
};
use chrono::Utc;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;
use uuid::Uuid;

use super::{ProjectId, QueryProject};
use crate::{
    context::DbConnection,
    error::{api_error, resource_not_found},
    schema,
    schema::testbed as testbed_table,
    util::{
        query::{fn_get, fn_get_id},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(TestbedId);

fn_resource_id!(testbed);

#[derive(Queryable)]
pub struct QueryTestbed {
    pub id: TestbedId,
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl QueryTestbed {
    fn_get!(testbed);
    fn_get_id!(testbed, TestbedId);

    pub fn get_uuid(conn: &mut DbConnection, id: TestbedId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::testbed::table
            .filter(schema::testbed::id.eq(id))
            .select(schema::testbed::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: Uuid,
    ) -> Result<Self, ApiError> {
        schema::testbed::table
            .filter(schema::testbed::project_id.eq(project_id))
            .filter(schema::testbed::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::testbed::table
            .filter(schema::testbed::project_id.eq(project_id))
            .filter(resource_id(testbed)?)
            .first::<Self>(conn)
            .map_err(resource_not_found!(Testbed, testbed.clone()))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonTestbed, ApiError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        Ok(JsonTestbed {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: NonEmpty::from_str(&name)?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
            created: to_date_time(created).map_err(api_error!())?,
            modified: to_date_time(modified).map_err(api_error!())?,
        })
    }

    pub fn is_system(&self) -> bool {
        matches!(self.name.as_ref(), TESTBED_LOCALHOST_STR)
            || matches!(self.slug.as_ref(), TESTBED_LOCALHOST_STR)
    }
}

#[derive(Insertable)]
#[diesel(table_name = testbed_table)]
pub struct InsertTestbed {
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut DbConnection,
        project: &ResourceId,
        testbed: JsonNewTestbed,
    ) -> Result<Self, ApiError> {
        let project_id = QueryProject::from_resource_id(conn, project)?.id;
        Ok(Self::from_json_inner(conn, project_id, testbed))
    }

    pub fn localhost(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json_inner(conn, project_id, JsonNewTestbed::localhost())
    }

    fn from_json_inner(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: JsonNewTestbed,
    ) -> Self {
        let JsonNewTestbed { name, slug } = testbed;
        let slug = unwrap_child_slug!(conn, project_id, name.as_ref(), slug, testbed, QueryTestbed);
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name: name.into(),
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = testbed_table)]
pub struct UpdateTestbed {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub modified: i64,
}

impl From<JsonUpdateTestbed> for UpdateTestbed {
    fn from(update: JsonUpdateTestbed) -> Self {
        let JsonUpdateTestbed { name, slug } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            modified: Utc::now().timestamp(),
        }
    }
}
