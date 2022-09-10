use bencher_json::report::JsonLatency;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::latency as latency_table, util::map_http_error};

#[derive(Queryable, Debug)]
pub struct QueryLatency {
    pub id: i32,
    pub uuid: String,
    pub lower_variance: i64,
    pub upper_variance: i64,
    pub duration: i64,
}

impl QueryLatency {
    pub fn to_json(self) -> Result<JsonLatency, HttpError> {
        let Self {
            id: _,
            uuid: _,
            lower_variance,
            upper_variance,
            duration,
        } = self;
        Ok(JsonLatency {
            lower_variance: lower_variance as u64,
            upper_variance: upper_variance as u64,
            duration: duration as u64,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = latency_table)]
pub struct InsertLatency {
    pub uuid: String,
    pub lower_variance: i64,
    pub upper_variance: i64,
    pub duration: i64,
}

impl From<JsonLatency> for InsertLatency {
    fn from(latency: JsonLatency) -> Self {
        let JsonLatency {
            lower_variance,
            upper_variance,
            duration,
        } = latency;
        Self {
            uuid: Uuid::new_v4().to_string(),
            lower_variance: lower_variance as i64,
            upper_variance: upper_variance as i64,
            duration: duration as i64,
        }
    }
}

impl InsertLatency {
    pub fn map_json(
        conn: &mut SqliteConnection,
        latency: Option<JsonLatency>,
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
