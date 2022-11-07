use std::str::FromStr;

use bencher_json::JsonBenchmark;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use uuid::Uuid;

use super::QueryProject;
use crate::{error::api_error, schema, schema::benchmark as benchmark_table, ApiError};

#[derive(Queryable)]
pub struct QueryBenchmark {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
}

impl QueryBenchmark {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, ApiError> {
        schema::benchmark::table
            .filter(schema::benchmark::uuid.eq(uuid.to_string()))
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(api_error!())
    }

    pub fn get_id_from_name(
        conn: &mut SqliteConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, ApiError> {
        schema::benchmark::table
            .filter(
                schema::benchmark::project_id
                    .eq(project_id)
                    .and(schema::benchmark::name.eq(name)),
            )
            .select(schema::benchmark::id)
            .first(conn)
            .map_err(api_error!())
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::benchmark::table
            .filter(schema::benchmark::id.eq(id))
            .select(schema::benchmark::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonBenchmark, ApiError> {
        let QueryBenchmark {
            id: _,
            uuid,
            project_id,
            name,
        } = self;
        Ok(JsonBenchmark {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name,
        })
    }

    pub fn get_or_create(
        conn: &mut SqliteConnection,
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

        QueryBenchmark::get_id(conn, insert_benchmark.uuid)
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
