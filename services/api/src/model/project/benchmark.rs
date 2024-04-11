use bencher_json::{
    project::benchmark::{JsonBenchmarkMetric, JsonNewBenchmark, JsonUpdateBenchmark},
    BenchmarkName, BenchmarkUuid, DateTime, JsonBenchmark, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{metric::QueryMetric, threshold::boundary::QueryBoundary, ProjectId, QueryProject};
use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
    schema,
    schema::benchmark as benchmark_table,
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

crate::util::typed_id::typed_id!(BenchmarkId);

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
    fn_eq_resource_id!(benchmark);
    fn_from_resource_id!(benchmark, Benchmark);

    fn_get!(benchmark, BenchmarkId);
    fn_get_id!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_get_uuid!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_from_uuid!(benchmark, BenchmarkUuid, Benchmark);

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

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: BenchmarkName,
    ) -> Result<BenchmarkId, HttpError> {
        // For historical reasons, we will only every be able to match on name and not name ID here.
        // The benchmark slugs were always created with a random suffix for a while.
        // Therefore, a name that happens to be a valid slug will fail to be found, when treated as a slug.
        if let Ok(id) = Self::get_id_from_name(conn, project_id, &name) {
            return Ok(id);
        }

        let insert_benchmark = InsertBenchmark::from_name(conn, project_id, name)?;
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
        self,
        project: &QueryProject,
        query_metric: QueryMetric,
        query_boundary: Option<QueryBoundary>,
    ) -> JsonBenchmarkMetric {
        let JsonBenchmark {
            uuid,
            project,
            name,
            slug,
            created,
            modified,
        } = self.into_json_for_project(project);
        let metric = query_metric.into_json();
        let boundary = query_boundary.map(QueryBoundary::into_json);
        JsonBenchmarkMetric {
            uuid,
            project,
            name,
            slug,
            metric,
            boundary,
            created,
            modified,
        }
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
    ) -> Result<Self, HttpError> {
        let JsonNewBenchmark { name, slug } = benchmark;
        let slug = ok_slug!(conn, project_id, &name, slug, benchmark, QueryBenchmark)?;
        Ok(Self::new(project_id, name, slug))
    }

    fn from_name(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: BenchmarkName,
    ) -> Result<Self, HttpError> {
        let slug = ok_slug!(conn, project_id, &name, None, benchmark, QueryBenchmark)?;
        Ok(Self::new(project_id, name, slug))
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
