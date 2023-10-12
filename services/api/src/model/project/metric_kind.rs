use bencher_json::{
    project::metric_kind::{
        JsonUpdateMetricKind, MetricKindUuid, ESTIMATED_CYCLES_NAME_STR, ESTIMATED_CYCLES_SLUG_STR,
        INSTRUCTIONS_NAME_STR, INSTRUCTIONS_SLUG_STR, L1_ACCESSES_NAME_STR, L1_ACCESSES_SLUG_STR,
        L2_ACCESSES_NAME_STR, L2_ACCESSES_SLUG_STR, LATENCY_NAME_STR, LATENCY_SLUG_STR,
        RAM_ACCESSES_NAME_STR, RAM_ACCESSES_SLUG_STR, THROUGHPUT_NAME_STR, THROUGHPUT_SLUG_STR,
    },
    JsonMetricKind, JsonNewMetricKind, NonEmpty, ResourceId, Slug,
};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{resource_insert_err, resource_not_found_err},
    model::project::QueryProject,
    schema,
    schema::metric_kind as metric_kind_table,
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
        to_date_time,
    },
    ApiError,
};

use super::{ProjectId, ProjectUuid};

crate::util::typed_id::typed_id!(MetricKindId);

fn_resource_id!(metric_kind);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable)]
#[diesel(table_name = metric_kind_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryMetricKind {
    pub id: MetricKindId,
    pub uuid: MetricKindUuid,
    pub project_id: ProjectId,
    pub name: NonEmpty,
    pub slug: Slug,
    pub units: NonEmpty,
    pub created: i64,
    pub modified: i64,
}

impl QueryMetricKind {
    fn_get!(metric_kind, MetricKindId);
    fn_get_id!(metric_kind, MetricKindId, MetricKindUuid);
    fn_get_uuid!(metric_kind, MetricKindId, MetricKindUuid);

    pub fn from_resource_id(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::metric_kind::table
            .filter(schema::metric_kind::project_id.eq(project_id))
            .filter(resource_id(metric_kind)?)
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(MetricKind, metric_kind.clone()))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonMetricKind, ApiError> {
        let project_uuid = QueryProject::get_uuid(conn, self.project_id)?;
        self.into_json_for_project(project_uuid)
    }

    pub fn into_json_for_project(
        self,
        project_uuid: ProjectUuid,
    ) -> Result<JsonMetricKind, ApiError> {
        let Self {
            uuid,
            name,
            slug,
            units,
            created,
            modified,
            ..
        } = self;
        Ok(JsonMetricKind {
            uuid,
            project: project_uuid,
            name,
            slug,
            units,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

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
        };
        diesel::insert_into(schema::metric_kind::table)
            .values(&insert_metric_kind)
            .execute(conn)
            .map_err(resource_insert_err!(MetricKind, insert_metric_kind))?;

        Self::get_id(conn, insert_metric_kind.uuid)
    }

    pub fn is_system(&self) -> bool {
        is_system(self.name.as_ref(), self.slug.as_ref())
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
    pub created: i64,
    pub modified: i64,
}

impl InsertMetricKind {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind: JsonNewMetricKind,
    ) -> Self {
        let JsonNewMetricKind { name, slug, units } = metric_kind;
        let slug = unwrap_child_slug!(
            conn,
            project_id,
            name.as_ref(),
            slug,
            metric_kind,
            QueryMetricKind
        );
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: MetricKindUuid::new(),
            project_id,
            name,
            slug,
            units,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn latency(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::latency())
    }

    pub fn throughput(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::throughput())
    }

    pub fn instructions(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::instructions())
    }

    pub fn l1_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::l1_accesses())
    }

    pub fn l2_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::l2_accesses())
    }

    pub fn ram_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewMetricKind::ram_accesses())
    }

    pub fn estimated_cycles(conn: &mut DbConnection, project_id: ProjectId) -> Self {
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
    pub modified: i64,
}

impl From<JsonUpdateMetricKind> for UpdateMetricKind {
    fn from(update: JsonUpdateMetricKind) -> Self {
        let JsonUpdateMetricKind { name, slug, units } = update;
        Self {
            name,
            slug,
            units,
            modified: Utc::now().timestamp(),
        }
    }
}
