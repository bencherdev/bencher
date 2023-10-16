use bencher_json::{
    project::metric_kind::{
        JsonUpdateMetricKind, MetricKindUuid, ESTIMATED_CYCLES_NAME_STR, ESTIMATED_CYCLES_SLUG_STR,
        INSTRUCTIONS_NAME_STR, INSTRUCTIONS_SLUG_STR, L1_ACCESSES_NAME_STR, L1_ACCESSES_SLUG_STR,
        L2_ACCESSES_NAME_STR, L2_ACCESSES_SLUG_STR, LATENCY_NAME_STR, LATENCY_SLUG_STR,
        RAM_ACCESSES_NAME_STR, RAM_ACCESSES_SLUG_STR, THROUGHPUT_NAME_STR, THROUGHPUT_SLUG_STR,
    },
    DateTime, JsonMetricKind, JsonNewMetricKind, NonEmpty, ResourceId, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_conflict_err, BencherResource},
    model::project::QueryProject,
    schema,
    schema::metric_kind as metric_kind_table,
    util::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_from_resource_id, fn_resource_id},
        slug::ok_slug,
    },
};

use super::ProjectId;

crate::util::typed_id::typed_id!(MetricKindId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = metric_kind_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryMetricKind {
    pub id: MetricKindId,
    pub uuid: MetricKindUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub units: NonEmpty,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryMetricKind {
    fn_resource_id!(metric_kind);
    fn_from_resource_id!(metric_kind, MetricKind);

    fn_get!(metric_kind, MetricKindId);
    fn_get_id!(metric_kind, MetricKindId, MetricKindUuid);
    fn_get_uuid!(metric_kind, MetricKindId, MetricKindUuid);

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind: &ResourceId,
    ) -> Result<MetricKindId, HttpError> {
        let query_metric_kind = Self::from_resource_id(conn, project_id, metric_kind);

        let http_error = match query_metric_kind {
            Ok(metric_kind) => return Ok(metric_kind.id),
            Err(e) => e,
        };

        // Dynamically create adapter specific metric kinds
        // Or recreate default metric kinds if they were previously deleted
        let insert_metric_kind = match metric_kind.as_ref() {
            // Recreate
            LATENCY_SLUG_STR => InsertMetricKind::latency(conn, project_id),
            THROUGHPUT_SLUG_STR => InsertMetricKind::throughput(conn, project_id),
            // Adapter specific
            INSTRUCTIONS_SLUG_STR => InsertMetricKind::instructions(conn, project_id),
            L1_ACCESSES_SLUG_STR => InsertMetricKind::l1_accesses(conn, project_id),
            L2_ACCESSES_SLUG_STR => InsertMetricKind::l2_accesses(conn, project_id),
            RAM_ACCESSES_SLUG_STR => InsertMetricKind::ram_accesses(conn, project_id),
            ESTIMATED_CYCLES_SLUG_STR => InsertMetricKind::estimated_cycles(conn, project_id),
            _ => return Err(http_error),
        }?;
        diesel::insert_into(schema::metric_kind::table)
            .values(&insert_metric_kind)
            .execute(conn)
            .map_err(resource_conflict_err!(MetricKind, insert_metric_kind))?;

        Self::get_id(conn, insert_metric_kind.uuid)
    }

    pub fn is_system(&self) -> bool {
        is_system(self.name.as_ref(), self.slug.as_ref())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonMetricKind, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        Ok(self.into_json_for_project(&project))
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonMetricKind {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            units,
            created,
            modified,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::MetricKind,
            project_id,
        );
        JsonMetricKind {
            uuid,
            project: project.uuid,
            name,
            slug,
            units,
            created,
            modified,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = metric_kind_table)]
pub struct InsertMetricKind {
    pub uuid: MetricKindUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub units: NonEmpty,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertMetricKind {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind: JsonNewMetricKind,
    ) -> Result<Self, HttpError> {
        let JsonNewMetricKind { name, slug, units } = metric_kind;
        let slug = ok_slug!(conn, project_id, &name, slug, metric_kind, QueryMetricKind)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: MetricKindUuid::new(),
            project_id,
            name,
            slug,
            units,
            created: timestamp,
            modified: timestamp,
        })
    }

    pub fn latency(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::latency())
    }

    pub fn throughput(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::throughput())
    }

    pub fn instructions(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::instructions())
    }

    pub fn l1_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::l1_accesses())
    }

    pub fn l2_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::l2_accesses())
    }

    pub fn ram_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::ram_accesses())
    }

    pub fn estimated_cycles(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMetricKind::estimated_cycles())
    }

    pub fn is_system(&self) -> bool {
        is_system(self.name.as_ref(), self.slug.as_ref())
    }
}

fn is_system(name: &str, slug: &str) -> bool {
    matches!(
        name,
        LATENCY_NAME_STR
            | THROUGHPUT_NAME_STR
            | INSTRUCTIONS_NAME_STR
            | L1_ACCESSES_NAME_STR
            | L2_ACCESSES_NAME_STR
            | RAM_ACCESSES_NAME_STR
            | ESTIMATED_CYCLES_NAME_STR
    ) || matches!(
        slug,
        LATENCY_SLUG_STR
            | THROUGHPUT_SLUG_STR
            | INSTRUCTIONS_SLUG_STR
            | L1_ACCESSES_SLUG_STR
            | L2_ACCESSES_SLUG_STR
            | RAM_ACCESSES_SLUG_STR
            | ESTIMATED_CYCLES_SLUG_STR
    )
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = metric_kind_table)]
pub struct UpdateMetricKind {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub units: Option<NonEmpty>,
    pub modified: DateTime,
}

impl From<JsonUpdateMetricKind> for UpdateMetricKind {
    fn from(update: JsonUpdateMetricKind) -> Self {
        let JsonUpdateMetricKind { name, slug, units } = update;
        Self {
            name,
            slug,
            units,
            modified: DateTime::now(),
        }
    }
}
