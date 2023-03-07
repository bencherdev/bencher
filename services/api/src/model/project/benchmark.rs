use std::str::FromStr;

use bencher_json::{BenchmarkName, JsonBenchmark};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::QueryProject;
use crate::{
    context::DbConnection, error::api_error, schema, schema::benchmark as benchmark_table,
    util::query::fn_get_id, ApiError,
};

#[derive(Queryable)]
pub struct QueryBenchmark {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
}

impl QueryBenchmark {
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

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBenchmark, ApiError> {
        let QueryBenchmark {
            uuid,
            project_id,
            name,
            ..
        } = self;
        Ok(JsonBenchmark {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: BenchmarkName::from_str(&name).map_err(api_error!())?,
        })
    }

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, ApiError> {
        let id = QueryBenchmark::get_id_from_name(conn, project_id, name);

        if id.is_ok() {
            return id;
        }

        let insert_benchmark = InsertBenchmark::from_json(project_id, name.to_string());
        diesel::insert_into(schema::benchmark::table)
            .values(&insert_benchmark)
            .execute(conn)
            .map_err(api_error!())?;

        QueryBenchmark::get_id(conn, &insert_benchmark.uuid)
    }
}

#[derive(Insertable)]
#[diesel(table_name = benchmark_table)]
pub struct InsertBenchmark {
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
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
