use bencher_json::report::JsonThroughput;
use diesel::{Insertable, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::{schema, schema::throughput as throughput_table},
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    util::http_error,
};

#[derive(Queryable, Debug)]
pub struct QueryThroughput {
    pub id: i32,
    pub uuid: String,
    pub lower_variance: f64,
    pub upper_variance: f64,
    pub events: f64,
    pub unit_time: i64,
}

impl QueryThroughput {
    pub fn to_json(self) -> Result<JsonThroughput, HttpError> {
        let Self {
            id: _,
            uuid: _,
            lower_variance,
            upper_variance,
            events,
            unit_time,
        } = self;
        Ok(JsonThroughput {
            lower_variance: lower_variance.into(),
            upper_variance: upper_variance.into(),
            events: events.into(),
            unit_time: unit_time as u64,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = throughput_table)]
pub struct InsertThroughput {
    pub uuid: String,
    pub lower_variance: f64,
    pub upper_variance: f64,
    pub events: f64,
    pub unit_time: i64,
}

impl From<JsonThroughput> for InsertThroughput {
    fn from(throughput: JsonThroughput) -> Self {
        let JsonThroughput {
            lower_variance,
            upper_variance,
            events,
            unit_time,
        } = throughput;
        Self {
            uuid: Uuid::new_v4().to_string(),
            lower_variance: lower_variance.into(),
            upper_variance: upper_variance.into(),
            events: events.into(),
            unit_time: unit_time as i64,
        }
    }
}

impl InsertThroughput {
    pub fn map_json(
        conn: &mut SqliteConnection,
        throughput: Option<JsonThroughput>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_throughput) = throughput {
            let insert_throughput: InsertThroughput = json_throughput.into();
            diesel::insert_into(schema::throughput::table)
                .values(&insert_throughput)
                .execute(conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::throughput::table
                    .filter(schema::throughput::uuid.eq(&insert_throughput.uuid))
                    .select(schema::throughput::id)
                    .first::<i32>(conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
