use std::str::FromStr;

use bencher_json::{
    project::benchmark::{JsonBenchmarkMetric, JsonNewBenchmark, JsonUpdateBenchmark},
    BenchmarkName, JsonBenchmark, ResourceId, Slug,
};
use chrono::Utc;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    metric::{MetricId, QueryMetric},
    threshold::boundary::QueryBoundary,
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema,
    schema::benchmark as benchmark_table,
    util::{
        query::{fn_get, fn_get_id},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(BenchmarkId);

fn_resource_id!(benchmark);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = benchmark_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryBenchmark {
    pub id: BenchmarkId,
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl QueryBenchmark {
    fn_get!(benchmark);
    fn_get_id!(benchmark, BenchmarkId);

    pub fn get_id_from_name(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: &str,
    ) -> Result<BenchmarkId, ApiError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::name.eq(name))
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(ApiError::from)
    }

    pub fn get_uuid(conn: &mut DbConnection, id: BenchmarkId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::benchmark::table
            .filter(schema::benchmark::id.eq(id))
            .select(schema::benchmark::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: Uuid,
    ) -> Result<Self, ApiError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(ApiError::from)
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        project_id: ProjectId,
        benchmark: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(resource_id(benchmark)?)
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Benchmark, benchmark.clone()))
    }

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: &str,
    ) -> Result<BenchmarkId, ApiError> {
        let id = Self::get_id_from_name(conn, project_id, name);

        if id.is_ok() {
            return id;
        }

        let insert_benchmark = InsertBenchmark::from_name(project_id, name.to_owned());
        diesel::insert_into(schema::benchmark::table)
            .values(&insert_benchmark)
            .execute(conn)
            .map_err(ApiError::from)?;

        Self::get_id(conn, &insert_benchmark.uuid)
    }

    fn get_benchmark_json(
        conn: &mut DbConnection,
        uuid: &str,
        project_id: ProjectId,
        name: &str,
        slug: &str,
        created: i64,
        modified: i64,
    ) -> Result<JsonBenchmark, ApiError> {
        Ok(JsonBenchmark {
            uuid: Uuid::from_str(uuid).map_err(ApiError::from)?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: BenchmarkName::from_str(name).map_err(ApiError::from)?,
            slug: Slug::from_str(slug).map_err(ApiError::from)?,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

    pub fn get_benchmark_metric_json(
        conn: &mut DbConnection,
        metric_id: MetricId,
    ) -> Result<JsonBenchmarkMetric, ApiError> {
        let (uuid, project_id, name, slug, created, modified, value, lower_value, upper_value) =
            schema::metric::table
                .filter(schema::metric::id.eq(metric_id))
                .left_join(schema::perf::table.on(schema::perf::id.eq(schema::metric::perf_id)))
                .inner_join(
                    schema::benchmark::table
                        .on(schema::benchmark::id.eq(schema::perf::benchmark_id)),
                )
                .select((
                    schema::benchmark::uuid,
                    schema::benchmark::project_id,
                    schema::benchmark::name,
                    schema::benchmark::slug,
                    schema::benchmark::created,
                    schema::benchmark::modified,
                    schema::metric::value,
                    schema::metric::lower_value,
                    schema::metric::upper_value,
                ))
                .first::<(
                    String,
                    ProjectId,
                    String,
                    String,
                    i64,
                    i64,
                    f64,
                    Option<f64>,
                    Option<f64>,
                )>(conn)
                .map_err(ApiError::from)?;

        let JsonBenchmark {
            uuid,
            project,
            name,
            slug,
            created,
            modified,
        } = Self::get_benchmark_json(conn, &uuid, project_id, &name, &slug, created, modified)?;
        let metric = QueryMetric::json(value, lower_value, upper_value);
        let boundary = QueryBoundary::get_json(conn, metric_id);

        Ok(JsonBenchmarkMetric {
            uuid,
            project,
            name,
            slug,
            metric,
            boundary,
            created,
            modified,
        })
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBenchmark, ApiError> {
        let QueryBenchmark {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        Self::get_benchmark_json(conn, &uuid, project_id, &name, &slug, created, modified)
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = benchmark_table)]
pub struct InsertBenchmark {
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl InsertBenchmark {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        benchmark: JsonNewBenchmark,
    ) -> Self {
        let JsonNewBenchmark { name, slug } = benchmark;
        let slug = unwrap_child_slug!(
            conn,
            project_id,
            name.as_ref(),
            slug,
            benchmark,
            QueryBenchmark
        );
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

    fn from_name(project_id: ProjectId, name: String) -> Self {
        let slug = format!(
            "{slug}-{rand_suffix}",
            slug = slug::slugify(&name),
            rand_suffix = rand::random::<u32>()
        );
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = benchmark_table)]
pub struct UpdateBenchmark {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub modified: i64,
}

impl From<JsonUpdateBenchmark> for UpdateBenchmark {
    fn from(update: JsonUpdateBenchmark) -> Self {
        let JsonUpdateBenchmark { name, slug } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            modified: Utc::now().timestamp(),
        }
    }
}
