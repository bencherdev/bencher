use std::str::FromStr;

use bencher_json::{JsonMetricKind, JsonNewMetricKind, NonEmpty, ResourceId, Slug};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use crate::{
    error::api_error,
    model::project::QueryProject,
    schema,
    schema::metric_kind as metric_kind_table,
    util::{query::fn_get_id, resource_id::fn_resource_id, slug::unwrap_child_slug},
    ApiError,
};

fn_resource_id!(metric_kind);

#[derive(Queryable)]
pub struct QueryMetricKind {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
    pub units: String,
}

impl QueryMetricKind {
    fn_get_id!(metric_kind);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::metric_kind::table
            .filter(schema::metric_kind::id.eq(id))
            .select(schema::metric_kind::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        project_id: i32,
        metric_kind: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::metric_kind::table
            .filter(schema::metric_kind::project_id.eq(project_id))
            .filter(resource_id(metric_kind)?)
            .first::<QueryMetricKind>(conn)
            .map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonMetricKind, ApiError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            units,
            ..
        } = self;
        Ok(JsonMetricKind {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: NonEmpty::from_str(&name).map_err(api_error!())?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
            units: NonEmpty::from_str(&units).map_err(api_error!())?,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = metric_kind_table)]
pub struct InsertMetricKind {
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
    pub units: String,
}

impl InsertMetricKind {
    pub fn from_json(
        conn: &mut SqliteConnection,
        project: &ResourceId,
        metric_kind: JsonNewMetricKind,
    ) -> Result<Self, ApiError> {
        let project_id = QueryProject::from_resource_id(conn, project)?.id;
        Ok(Self::from_json_inner(conn, project_id, metric_kind))
    }

    pub fn latency(conn: &mut SqliteConnection, project_id: i32) -> Self {
        Self::from_json_inner(conn, project_id, JsonNewMetricKind::latency())
    }

    pub fn from_json_inner(
        conn: &mut SqliteConnection,
        project_id: i32,
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
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name: name.into(),
            slug,
            units: units.into(),
        }
    }
}
