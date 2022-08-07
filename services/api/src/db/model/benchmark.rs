use std::str::FromStr;

use bencher_json::report::{
    JsonBenchmarkPerf,
    JsonBenchmarks,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
    Insertable,
    JoinOnDsl,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::benchmark as benchmark_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

const BENCHMARK_ERROR: &str = "Failed to get benchmark.";

#[derive(Queryable)]
pub struct QueryBenchmark {
    pub id:         i32,
    pub uuid:       String,
    pub project_id: i32,
    pub name:       String,
}

impl QueryBenchmark {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::uuid.eq(uuid.to_string()))
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(|_| http_error!(BENCHMARK_ERROR))
    }

    pub fn get_id_from_name(
        conn: &SqliteConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, HttpError> {
        schema::benchmark::table
            .filter(
                schema::benchmark::project_id
                    .eq(project_id)
                    .and(schema::benchmark::name.eq(name)),
            )
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(|_| http_error!(BENCHMARK_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::benchmark::table
            .filter(schema::benchmark::id.eq(id))
            .select(schema::benchmark::uuid)
            .first(conn)
            .map_err(|_| http_error!(BENCHMARK_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(BENCHMARK_ERROR))
    }

    pub fn get_benchmarks(
        conn: &SqliteConnection,
        report_id: i32,
    ) -> Result<JsonBenchmarks, HttpError> {
        let uuids: Vec<(String, String)> = schema::perf::table
            .inner_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::perf::report_id.eq(report_id))
            .select((schema::benchmark::uuid, schema::perf::uuid))
            .order(schema::benchmark::name)
            .load::<(String, String)>(conn)
            .map_err(|_| http_error!(BENCHMARK_ERROR))?;

        let mut benchmarks = JsonBenchmarks::new();
        for (benchmark_uuid, perf_uuid) in uuids {
            benchmarks.push(JsonBenchmarkPerf {
                benchmark_uuid: Uuid::from_str(&benchmark_uuid)
                    .map_err(|_| http_error!(BENCHMARK_ERROR))?,
                perf_uuid:      Uuid::from_str(&perf_uuid)
                    .map_err(|_| http_error!(BENCHMARK_ERROR))?,
            });
        }

        Ok(benchmarks)
    }

    pub fn to_json(self, conn: &SqliteConnection) -> Result<(), HttpError> {
        todo!()
    }
}

#[derive(Insertable)]
#[table_name = "benchmark_table"]
pub struct InsertBenchmark {
    pub uuid:       String,
    pub project_id: i32,
    pub name:       String,
}

impl InsertBenchmark {
    pub fn new(project_id: i32, name: String) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name,
        }
    }
}
