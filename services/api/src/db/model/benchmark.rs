use std::str::FromStr;

use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
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

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryBenchmark {
    pub id:         i32,
    pub uuid:       String,
    pub project_id: i32,
    pub name:       String,
}

impl QueryBenchmark {
    pub fn get_id(conn: &SqliteConnection, uuid: Uuid) -> Result<i32, HttpError> {
        schema::benchmark::table
            .filter(schema::benchmark::uuid.eq(&uuid.to_string()))
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

    pub fn to_json(self) -> String {
        self.name
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
