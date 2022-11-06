use bencher_json::project::report::JsonMetric;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::latency as latency_table, util::map_http_error};

#[derive(Queryable, Debug)]
pub struct QueryMetric {
    pub id: i32,
    pub uuid: String,
    pub value: i64,
    pub lower_bound: Option<i64>,
    pub upper_bound: Option<i64>,
}

impl QueryMetric {
    pub fn into_json(self) -> Result<JsonMetric, HttpError> {
        let Self {
            id: _,
            uuid: _,
            lower_bound,
            upper_bound,
            duration,
        } = self;
        Ok(JsonMetric {
            lower_bound: lower_bound as u64,
            upper_bound: upper_bound as u64,
            duration: duration as u64,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = latency_table)]
pub struct InsertLatency {
    pub uuid: String,
    pub lower_bound: i64,
    pub upper_bound: i64,
    pub duration: i64,
}

impl From<JsonMetric> for InsertLatency {
    fn from(latency: JsonMetric) -> Self {
        let JsonMetric {
            lower_bound,
            upper_bound,
            duration,
        } = latency;
        Self {
            uuid: Uuid::new_v4().to_string(),
            lower_bound: lower_bound as i64,
            upper_bound: upper_bound as i64,
            duration: duration as i64,
        }
    }
}

impl InsertLatency {
    pub fn map_json(
        conn: &mut SqliteConnection,
        latency: Option<JsonMetric>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_latency) = latency {
            let insert_latency: InsertLatency = json_latency.into();
            diesel::insert_into(schema::latency::table)
                .values(&insert_latency)
                .execute(conn)
                .map_err(map_http_error!("Failed to create benchmark data."))?;

            Some(
                schema::latency::table
                    .filter(schema::latency::uuid.eq(&insert_latency.uuid))
                    .select(schema::latency::id)
                    .first::<i32>(conn)
                    .map_err(map_http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
