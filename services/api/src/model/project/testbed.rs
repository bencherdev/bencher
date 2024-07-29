use bencher_json::{
    project::testbed::JsonUpdateTestbed, DateTime, JsonNewTestbed, JsonTestbed, NameId, NameIdKind,
    ResourceName, Slug, TestbedUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_conflict_err, BencherResource},
    schema::{self, testbed as testbed_table},
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
    pub archived: Option<DateTime>,
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

    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &NameId,
    ) -> Result<TestbedId, HttpError> {
        let query_testbed = Self::get_or_create_inner(context, project_id, testbed).await?;

        if query_testbed.archived.is_some() {
            let update_testbed = UpdateTestbed::unarchive();
            diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(query_testbed.id)))
                .set(&update_testbed)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Testbed, &query_testbed))?;
        }

        Ok(query_testbed.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &NameId,
    ) -> Result<Self, HttpError> {
        let query_testbed = Self::from_name_id(conn_lock!(context), project_id, testbed);

        let http_error = match query_testbed {
            Ok(testbed) => return Ok(testbed),
            Err(e) => e,
        };

        let Ok(kind) = NameIdKind::<ResourceName>::try_from(testbed) else {
            return Err(http_error);
        };
        let testbed = match kind {
            NameIdKind::Uuid(_) => return Err(http_error),
            NameIdKind::Slug(slug) => JsonNewTestbed {
                name: slug.clone().into(),
                slug: Some(slug),
            },
            NameIdKind::Name(name) => JsonNewTestbed { name, slug: None },
        };
        let insert_testbed = InsertTestbed::from_json(conn_lock!(context), project_id, testbed)?;
        diesel::insert_into(schema::testbed::table)
            .values(&insert_testbed)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Testbed, insert_testbed))?;

        Self::from_uuid(conn_lock!(context), project_id, insert_testbed.uuid)
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonTestbed {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            archived,
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
            archived,
        }
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
    pub archived: Option<DateTime>,
}

impl InsertTestbed {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: JsonNewTestbed,
    ) -> Result<Self, HttpError> {
        let JsonNewTestbed { name, slug } = testbed;
        let slug = ok_slug!(conn, project_id, &name, slug, testbed, QueryTestbed)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: TestbedUuid::new(),
            project_id,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
            archived: None,
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
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateTestbed> for UpdateTestbed {
    fn from(update: JsonUpdateTestbed) -> Self {
        let JsonUpdateTestbed {
            name,
            slug,
            archived,
        } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self {
            name,
            slug,
            modified,
            archived,
        }
    }
}

impl UpdateTestbed {
    fn unarchive() -> Self {
        JsonUpdateTestbed {
            name: None,
            slug: None,
            archived: Some(false),
        }
        .into()
    }
}
