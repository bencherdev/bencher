use bencher_json::{
    project::testbed::{JsonUpdateTestbed, TESTBED_LOCALHOST_STR},
    JsonNewTestbed, JsonTestbed, NonEmpty, ResourceId, Slug, TestbedUuid,
};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema,
    schema::testbed as testbed_table,
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(TestbedId);

fn_resource_id!(testbed);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = testbed_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryTestbed {
    pub id: TestbedId,
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub created: i64,
    pub modified: i64,
}

impl QueryTestbed {
    fn_get!(testbed, TestbedId);
    fn_get_id!(testbed, TestbedId, TestbedUuid);
    fn_get_uuid!(testbed, TestbedId, TestbedUuid);

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: TestbedUuid,
    ) -> Result<Self, HttpError> {
        schema::testbed::table
            .filter(schema::testbed::project_id.eq(project_id))
            .filter(schema::testbed::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Testbed, uuid))
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
            .map_err(resource_not_found_err!(Testbed, testbed.clone()))
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
            uuid,
            project: QueryProject::get_uuid(conn, project_id)?,
            name,
            slug,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

    pub fn is_system(&self) -> bool {
        matches!(self.name.as_ref(), TESTBED_LOCALHOST_STR)
            || matches!(self.slug.as_ref(), TESTBED_LOCALHOST_STR)
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = testbed_table)]
pub struct InsertTestbed {
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub created: i64,
    pub modified: i64,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: JsonNewTestbed,
    ) -> Self {
        let JsonNewTestbed { name, slug } = testbed;
        let slug = unwrap_child_slug!(conn, project_id, name.as_ref(), slug, testbed, QueryTestbed);
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: TestbedUuid::new(),
            project_id,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn localhost(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewTestbed::localhost())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = testbed_table)]
pub struct UpdateTestbed {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub modified: i64,
}

impl From<JsonUpdateTestbed> for UpdateTestbed {
    fn from(update: JsonUpdateTestbed) -> Self {
        let JsonUpdateTestbed { name, slug } = update;
        Self {
            name,
            slug,
            modified: Utc::now().timestamp(),
        }
    }
}
