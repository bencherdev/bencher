use std::str::FromStr;

use bencher_json::{project::benchmark::JsonBenchmarkMetric, BenchmarkName, JsonBenchmark};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::{metric::QueryMetric, threshold::boundary::QueryBoundary, QueryProject};
use crate::{
    context::DbConnection,
    error::api_error,
    schema,
    schema::benchmark as benchmark_table,
    util::query::{fn_get, fn_get_id},
    ApiError,
};

#[derive(Queryable)]
pub struct QueryBenchmark {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub created: i64,
}

impl QueryBenchmark {
    fn_get!(benchmark);
    fn_get_id!(benchmark);

    pub fn get_id_from_name(
        conn: &mut DbConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, ApiError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::name.eq(name))
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(api_error!())
    }

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::benchmark::table
            .filter(schema::benchmark::id.eq(id))
            .select(schema::benchmark::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: i32,
        uuid: Uuid,
    ) -> Result<Self, ApiError> {
        schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(api_error!())
    }

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, ApiError> {
        let id = Self::get_id_from_name(conn, project_id, name);

        if id.is_ok() {
            return id;
        }

        let insert_benchmark = InsertBenchmark::from_json(project_id, name.to_string());
        diesel::insert_into(schema::benchmark::table)
            .values(&insert_benchmark)
            .execute(conn)
            .map_err(api_error!())?;

        Self::get_id(conn, &insert_benchmark.uuid)
    }

    fn get_benchmark_json(
        conn: &mut DbConnection,
        uuid: String,
        project_id: i32,
        name: String,
    ) -> Result<JsonBenchmark, ApiError> {
        Ok(JsonBenchmark {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: BenchmarkName::from_str(&name).map_err(api_error!())?,
        })
    }

    pub fn get_benchmark_metric_json(
        conn: &mut DbConnection,
        metric_id: i32,
    ) -> Result<JsonBenchmarkMetric, ApiError> {
        let (uuid, project_id, name, value, lower_bound, upper_bound) = schema::metric::table
            .filter(schema::metric::id.eq(metric_id))
            .left_join(schema::perf::table.on(schema::perf::id.eq(schema::metric::perf_id)))
            .inner_join(
                schema::benchmark::table.on(schema::benchmark::id.eq(schema::perf::benchmark_id)),
            )
            .select((
                schema::benchmark::uuid,
                schema::benchmark::project_id,
                schema::benchmark::name,
                schema::metric::value,
                schema::metric::lower_bound,
                schema::metric::upper_bound,
            ))
            .first(conn)
            .map_err(api_error!())?;

        let JsonBenchmark {
            uuid,
            project,
            name,
        } = Self::get_benchmark_json(conn, uuid, project_id, name)?;
        let metric = QueryMetric::json(value, lower_bound, upper_bound);
        let boundary = QueryBoundary::get_json(conn, metric_id);

        Ok(JsonBenchmarkMetric {
            uuid,
            project,
            name,
            metric,
            boundary,
        })
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBenchmark, ApiError> {
        let QueryBenchmark {
            uuid,
            project_id,
            name,
            ..
        } = self;
        Self::get_benchmark_json(conn, uuid, project_id, name)
    }
}

#[derive(Insertable)]
#[diesel(table_name = benchmark_table)]
pub struct InsertBenchmark {
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub created: i64,
}

impl InsertBenchmark {
    pub fn from_json(project_id: i32, name: String) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name,
        }
    }
}
