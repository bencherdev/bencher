use bencher_json::{
    project::testbed::{JsonUpdateTestbed, TESTBED_LOCALHOST_STR},
    DateTime, JsonNewTestbed, JsonTestbed, NonEmpty, ResourceId, Slug, TestbedUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_not_found_err, BencherResource},
    schema,
    schema::testbed as testbed_table,
    util::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::ok_child_slug,
    },
};

crate::util::typed_id::typed_id!(TestbedId);

fn_resource_id!(testbed);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = testbed_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryTestbed {
    pub id: TestbedId,
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
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
            .map_err(resource_not_found_err!(Testbed, (project_id, uuid)))
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
            .map_err(resource_not_found_err!(Testbed, (project_id, testbed)))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonTestbed, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        Ok(self.into_json_for_project(&project))
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonTestbed {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Testbed,
            project_id,
        );
        JsonTestbed {
            uuid,
            project: project.uuid,
            name,
            slug,
            created,
            modified,
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(self.name.as_ref(), TESTBED_LOCALHOST_STR)
            || matches!(self.slug.as_ref(), TESTBED_LOCALHOST_STR)
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = testbed_table)]
pub struct InsertTestbed {
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: JsonNewTestbed,
    ) -> Result<Self, HttpError> {
        let JsonNewTestbed { name, slug } = testbed;
        let slug = ok_child_slug!(conn, project_id, &name, slug, testbed, QueryTestbed)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: TestbedUuid::new(),
            project_id,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        })
    }

    pub fn localhost(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewTestbed::localhost())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = testbed_table)]
pub struct UpdateTestbed {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub modified: DateTime,
}

impl From<JsonUpdateTestbed> for UpdateTestbed {
    fn from(update: JsonUpdateTestbed) -> Self {
        let JsonUpdateTestbed { name, slug } = update;
        Self {
            name,
            slug,
            modified: DateTime::now(),
        }
    }
}
