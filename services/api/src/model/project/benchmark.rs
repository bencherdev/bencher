use bencher_json::{
    project::benchmark::{JsonBenchmarkMetric, JsonNewBenchmark, JsonUpdateBenchmark},
    BenchmarkName, BenchmarkUuid, DateTime, JsonBenchmark, ResourceId, Slug,
};
use diesel::{
    ExpressionMethods, NullableExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper,
};
use dropshot::HttpError;

use super::{
    metric::{MetricId, QueryMetric},
    threshold::boundary::QueryBoundary,
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
    schema,
    schema::benchmark as benchmark_table,
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(BenchmarkId);

fn_resource_id!(benchmark);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = benchmark_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryBenchmark {
    pub id: BenchmarkId,
    pub uuid: BenchmarkUuid,
    pub project_id: ProjectId,
    pub name: BenchmarkName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryBenchmark {
    fn_get!(benchmark, BenchmarkId);
    fn_get_id!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_get_uuid!(benchmark, BenchmarkId, BenchmarkUuid);

    pub fn get_id_from_name(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: &BenchmarkName,
    ) -> Result<BenchmarkId, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::name.eq(name))
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(resource_not_found_err!(Benchmark, (project_id, name)))
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: BenchmarkUuid,
    ) -> Result<Self, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Benchmark, (project_id, uuid)))
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
            .map_err(resource_not_found_err!(Benchmark, (project_id, benchmark)))
    }

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: BenchmarkName,
    ) -> Result<BenchmarkId, HttpError> {
        if let Ok(id) = Self::get_id_from_name(conn, project_id, &name) {
            return Ok(id);
        }

        let insert_benchmark = InsertBenchmark::from_name(project_id, name);
        diesel::insert_into(schema::benchmark::table)
            .values(&insert_benchmark)
            .execute(conn)
            .map_err(resource_conflict_err!(Benchmark, insert_benchmark))?;

        Self::get_id(conn, insert_benchmark.uuid)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBenchmark, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        Ok(self.into_json_for_project(&project))
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonBenchmark {
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
            BencherResource::Benchmark,
            project_id,
        );
        JsonBenchmark {
            uuid,
            project: project.uuid,
            name,
            slug,
            created,
            modified,
        }
    }

    pub fn into_benchmark_metric_json(
        conn: &mut DbConnection,
        metric_id: MetricId,
    ) -> Result<JsonBenchmarkMetric, ApiError> {
        let (query_benchmark, query_metric, query_boundary) = schema::metric::table
            .filter(schema::metric::id.eq(metric_id))
            .inner_join(schema::perf::table.inner_join(schema::benchmark::table))
            // There may or may not be a boundary for any given metric
            .left_join(schema::boundary::table)
            .select((
                QueryBenchmark::as_select(),
                QueryMetric::as_select(),
                (
                    schema::boundary::id,
                    schema::boundary::uuid,
                    schema::boundary::threshold_id,
                    schema::boundary::statistic_id,
                    schema::boundary::metric_id,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                )
                    .nullable(),
            ))
            .first::<(QueryBenchmark, QueryMetric, Option<QueryBoundary>)>(conn)
            .map_err(ApiError::from)?;
        let project = QueryProject::get(conn, query_benchmark.project_id)?;
        query_benchmark.into_benchmark_metric_json_for_project(
            &project,
            query_metric,
            query_boundary,
        )
    }

    pub fn into_benchmark_metric_json_for_project(
        self,
        project: &QueryProject,
        query_metric: QueryMetric,
        query_boundary: Option<QueryBoundary>,
    ) -> Result<JsonBenchmarkMetric, ApiError> {
        let JsonBenchmark {
            uuid,
            project,
            name,
            slug,
            created,
            modified,
        } = self.into_json_for_project(project);
        let metric = query_metric.into_json();
        let boundary = query_boundary
            .map(QueryBoundary::into_json)
            .unwrap_or_default();
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
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = benchmark_table)]
pub struct InsertBenchmark {
    pub uuid: BenchmarkUuid,
    pub project_id: ProjectId,
    pub name: BenchmarkName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertBenchmark {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        benchmark: JsonNewBenchmark,
    ) -> Self {
        let JsonNewBenchmark { name, slug } = benchmark;
        let slug = unwrap_child_slug!(conn, project_id, &name, slug, benchmark, QueryBenchmark);
        Self::new(project_id, name, slug)
    }

    fn from_name(project_id: ProjectId, name: BenchmarkName) -> Self {
        let slug = Slug::new(&name);
        Self::new(project_id, name, slug)
    }

    fn new(project_id: ProjectId, name: BenchmarkName, slug: Slug) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid: BenchmarkUuid::new(),
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
    pub name: Option<BenchmarkName>,
    pub slug: Option<Slug>,
    pub modified: DateTime,
}

impl From<JsonUpdateBenchmark> for UpdateBenchmark {
    fn from(update: JsonUpdateBenchmark) -> Self {
        let JsonUpdateBenchmark { name, slug } = update;
        Self {
            name,
            slug,
            modified: DateTime::now(),
        }
    }
}
