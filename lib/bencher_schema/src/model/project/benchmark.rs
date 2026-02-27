use bencher_json::{
    BenchmarkName, BenchmarkNameId, BenchmarkSlug, BenchmarkUuid, DateTime, JsonBenchmark, NameId,
    project::benchmark::{JsonNewBenchmark, JsonUpdateBenchmark},
};
use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{BencherResource, assert_parentage, resource_conflict_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
        sql::last_insert_rowid,
    },
    schema::{self, benchmark as benchmark_table},
    write_conn,
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

    fn_eq_name_id!(ResourceName, benchmark, BenchmarkNameId);
    fn_from_name_id!(benchmark, Benchmark, BenchmarkNameId);

    fn_get!(benchmark, BenchmarkId);
    fn_get_id!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_get_uuid!(benchmark, BenchmarkId, BenchmarkUuid);
    fn_from_uuid!(project_id, ProjectId, benchmark, BenchmarkUuid, Benchmark);

    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark: &BenchmarkNameId,
    ) -> Result<BenchmarkId, HttpError> {
        let query_benchmark = Self::get_or_create_inner(context, project_id, benchmark).await?;

        if query_benchmark.archived.is_some() {
            let update_benchmark = UpdateBenchmark::unarchive();
            diesel::update(
                schema::benchmark::table.filter(schema::benchmark::id.eq(query_benchmark.id)),
            )
            .set(&update_benchmark)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Benchmark, &query_benchmark))?;
        }

        Ok(query_benchmark.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark: &BenchmarkNameId,
    ) -> Result<Self, HttpError> {
        let query_benchmark = Self::from_name_id(auth_conn!(context), project_id, benchmark);

        let http_error = match query_benchmark {
            Ok(benchmark) => return Ok(benchmark),
            Err(e) => e,
        };

        let json_benchmark = match benchmark.clone() {
            NameId::Uuid(_) => return Err(http_error),
            NameId::Slug(slug) => JsonNewBenchmark {
                name: slug.clone().into(),
                slug: Some(slug),
            },
            NameId::Name(name) => JsonNewBenchmark { name, slug: None },
        };

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
            InsertBenchmark::from_json(auth_conn!(context), project_id, json_benchmark);

        let conn = write_conn!(context);
        conn.transaction(|conn| {
            diesel::insert_into(schema::benchmark::table)
                .values(&insert_benchmark)
                .execute(conn)?;
            diesel::select(last_insert_rowid()).get_result(conn)
        })
        .map_err(resource_conflict_err!(Benchmark, &insert_benchmark))
        .map(|id| insert_benchmark.into_query(id))
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

    pub fn into_query(self, id: BenchmarkId) -> QueryBenchmark {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            archived,
        } = self;
        QueryBenchmark {
            id,
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            archived,
        }
    }

    fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        benchmark: JsonNewBenchmark,
    ) -> Self {
        let JsonNewBenchmark { name, slug } = benchmark;
        let slug = ok_slug!(conn, project_id, &name, slug, benchmark, QueryBenchmark);
        let timestamp = DateTime::now();
        Self {
            uuid: BenchmarkUuid::new(),
            project_id,
            name,
            slug,
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

#[cfg(test)]
mod tests {
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use super::BenchmarkId;
    use crate::{
        macros::sql::last_insert_rowid,
        schema,
        test_util::{create_base_entities, create_benchmark, setup_test_db},
    };

    #[test]
    fn last_insert_rowid_returns_benchmark_id() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::benchmark::table)
                    .values((
                        schema::benchmark::uuid.eq(uuid),
                        schema::benchmark::project_id.eq(base.project_id),
                        schema::benchmark::name.eq("Bench 1"),
                        schema::benchmark::slug.eq("bench-1"),
                        schema::benchmark::created.eq(DateTime::TEST),
                        schema::benchmark::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<BenchmarkId>(conn)?;
                let select_id: BenchmarkId = schema::benchmark::table
                    .filter(schema::benchmark::uuid.eq(uuid))
                    .select(schema::benchmark::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_benchmark() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Insert first
        diesel::insert_into(schema::benchmark::table)
            .values((
                schema::benchmark::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::benchmark::project_id.eq(base.project_id),
                schema::benchmark::name.eq("Bench 1"),
                schema::benchmark::slug.eq("bench-1"),
                schema::benchmark::created.eq(DateTime::TEST),
                schema::benchmark::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first benchmark");

        // Insert second + verify
        let second_uuid = "00000000-0000-0000-0000-000000000011";
        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::benchmark::table)
                    .values((
                        schema::benchmark::uuid.eq(second_uuid),
                        schema::benchmark::project_id.eq(base.project_id),
                        schema::benchmark::name.eq("Bench 2"),
                        schema::benchmark::slug.eq("bench-2"),
                        schema::benchmark::created.eq(DateTime::TEST),
                        schema::benchmark::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<BenchmarkId>(conn)?;
                let select_id: BenchmarkId = schema::benchmark::table
                    .filter(schema::benchmark::uuid.eq(second_uuid))
                    .select(schema::benchmark::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);

        let first_id: BenchmarkId = schema::benchmark::table
            .filter(schema::benchmark::uuid.eq("00000000-0000-0000-0000-000000000010"))
            .select(schema::benchmark::id)
            .first(&mut conn)
            .expect("Failed to get first benchmark id");
        assert_ne!(rowid, first_id);
    }

    #[test]
    fn benchmark_insert_and_readback_same_conn() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        // Insert and read back within same transaction
        let (inserted_id, readback_name) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::benchmark::table)
                    .values((
                        schema::benchmark::uuid.eq(uuid),
                        schema::benchmark::project_id.eq(base.project_id),
                        schema::benchmark::name.eq("Test Bench"),
                        schema::benchmark::slug.eq("test-bench"),
                        schema::benchmark::created.eq(DateTime::TEST),
                        schema::benchmark::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let id = diesel::select(last_insert_rowid()).get_result::<BenchmarkId>(conn)?;
                let name: String = schema::benchmark::table
                    .filter(schema::benchmark::uuid.eq(uuid))
                    .select(schema::benchmark::name)
                    .first(conn)?;

                diesel::QueryResult::Ok((id, name))
            })
            .expect("Transaction failed");

        assert_eq!(readback_name, "Test Bench");

        // Verify outside transaction
        let outside_id: BenchmarkId = schema::benchmark::table
            .filter(schema::benchmark::uuid.eq(uuid))
            .select(schema::benchmark::id)
            .first(&mut conn)
            .expect("Failed to read back");
        assert_eq!(inserted_id, outside_id);
    }

    #[test]
    fn benchmark_unarchive() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        // Initially not archived
        let archived: Option<i64> = schema::benchmark::table
            .filter(schema::benchmark::id.eq(benchmark_id))
            .select(schema::benchmark::archived)
            .first(&mut conn)
            .expect("Failed to query");
        assert!(archived.is_none());

        // Archive it
        diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(benchmark_id)))
            .set(schema::benchmark::archived.eq(Some(1i64)))
            .execute(&mut conn)
            .expect("Failed to archive");

        let archived: Option<i64> = schema::benchmark::table
            .filter(schema::benchmark::id.eq(benchmark_id))
            .select(schema::benchmark::archived)
            .first(&mut conn)
            .expect("Failed to query");
        assert!(archived.is_some());

        // Unarchive it
        diesel::update(schema::benchmark::table.filter(schema::benchmark::id.eq(benchmark_id)))
            .set(schema::benchmark::archived.eq(None::<i64>))
            .execute(&mut conn)
            .expect("Failed to unarchive");

        let archived: Option<i64> = schema::benchmark::table
            .filter(schema::benchmark::id.eq(benchmark_id))
            .select(schema::benchmark::archived)
            .first(&mut conn)
            .expect("Failed to query");
        assert!(archived.is_none());
    }
}
