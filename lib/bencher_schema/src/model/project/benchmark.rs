use bencher_json::{
    BenchmarkName, BenchmarkSlug, BenchmarkUuid, DateTime, JsonBenchmark,
    project::benchmark::{JsonNewBenchmark, JsonUpdateBenchmark},
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{BencherResource, assert_parentage, resource_conflict_err, resource_not_found_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    schema::{self, benchmark as benchmark_table},
};

crate::macros::typed_id::typed_id!(BenchmarkId);

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
    pub slug: BenchmarkSlug,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryBenchmark {
    fn_eq_resource_id!(benchmark, BenchmarkResourceId);
    fn_from_resource_id!(
        project_id,
        ProjectId,
        benchmark,
        Benchmark,
        BenchmarkResourceId
    );

    fn_get!(benchmark, BenchmarkId);
    fn_get_id!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_get_uuid!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_from_uuid!(benchmark, BenchmarkUuid, Benchmark);

    pub fn get_from_name(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: &BenchmarkName,
    ) -> Result<Self, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::name.eq(name))
            .first(conn)
            .map_err(resource_not_found_err!(Benchmark, (project_id, name)))
    }

    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        name: BenchmarkName,
    ) -> Result<BenchmarkId, HttpError> {
        let query_benchmark = Self::get_or_create_inner(context, project_id, name).await?;

        if query_benchmark.archived.is_some() {
            let update_benchmark = UpdateBenchmark::unarchive();
            diesel::update(
                schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)),
            )
            .set(&update_benchmark)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Benchmark, &query_benchmark))?;
        }

        Ok(query_benchmark.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        name: BenchmarkName,
    ) -> Result<Self, HttpError> {
        // For historical reasons, we will only every be able to match on name and not name ID here.
        // The benchmark slugs were always created with a random suffix for a while.
        // Therefore, a name that happens to be a valid slug will fail to be found, when treated as a slug.
        if let Ok(benchmark) = Self::get_from_name(conn_lock!(context), project_id, &name) {
            return Ok(benchmark);
        }

        let json_benchmark = JsonNewBenchmark { name, slug: None };
        Self::create(context, project_id, json_benchmark).await
    }

    pub async fn create(
        context: &ApiContext,
        project_id: ProjectId,
        json_benchmark: JsonNewBenchmark,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        InsertBenchmark::rate_limit(context, project_id).await?;

        let insert_benchmark =
            InsertBenchmark::from_json(conn_lock!(context), project_id, json_benchmark);
        diesel::insert_into(schema::benchmark::table)
            .values(&insert_benchmark)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Benchmark, &insert_benchmark))?;

        Self::from_uuid(conn_lock!(context), project_id, insert_benchmark.uuid)
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonBenchmark {
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
            archived,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = benchmark_table)]
pub struct InsertBenchmark {
    pub uuid: BenchmarkUuid,
    pub project_id: ProjectId,
    pub name: BenchmarkName,
    pub slug: BenchmarkSlug,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertBenchmark {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(benchmark, Benchmark);

    fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        benchmark: JsonNewBenchmark,
    ) -> Self {
        let JsonNewBenchmark { name, slug } = benchmark;
        let slug = ok_slug!(
            conn,
            project_id,
            &name,
            slug.map(Into::into),
            benchmark,
            QueryBenchmark
        );
        let timestamp = DateTime::now();
        Self {
            uuid: BenchmarkUuid::new(),
            project_id,
            name,
            slug: slug.into(),
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = benchmark_table)]
pub struct UpdateBenchmark {
    pub name: Option<BenchmarkName>,
    pub slug: Option<BenchmarkSlug>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateBenchmark> for UpdateBenchmark {
    fn from(update: JsonUpdateBenchmark) -> Self {
        let JsonUpdateBenchmark {
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

impl UpdateBenchmark {
    fn unarchive() -> Self {
        JsonUpdateBenchmark {
            name: None,
            slug: None,
            archived: Some(false),
        }
        .into()
    }
}
