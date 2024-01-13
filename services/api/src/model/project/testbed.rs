use bencher_json::{
    project::testbed::{JsonUpdateTestbed, TESTBED_LOCALHOST_STR},
    DateTime, JsonNewTestbed, JsonTestbed, ResourceName, Slug, TestbedUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    context::DbConnection,
    error::{assert_parentage, BencherResource},
    schema,
    schema::testbed as testbed_table,
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

crate::util::typed_id::typed_id!(TestbedId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = testbed_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryTestbed {
    pub id: TestbedId,
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryTestbed {
    fn_eq_resource_id!(testbed);
    fn_from_resource_id!(testbed, Testbed);

    fn_eq_name_id!(ResourceName, testbed);
    fn_from_name_id!(testbed, Testbed);

    fn_get!(testbed, TestbedId);
    fn_get_id!(testbed, TestbedId, TestbedUuid);
    fn_get_uuid!(testbed, TestbedId, TestbedUuid);
    fn_from_uuid!(testbed, TestbedUuid, Testbed);

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
    pub name: ResourceName,
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
        let JsonNewTestbed { name, slug, .. } = testbed;
        let slug = ok_slug!(conn, project_id, &name, slug, testbed, QueryTestbed)?;
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
    pub name: Option<ResourceName>,
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
