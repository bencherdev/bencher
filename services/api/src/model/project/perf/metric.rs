use bencher_json::project::report::JsonMetric;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::metric as metric_table, util::map_http_error};

#[derive(Queryable, Debug)]
pub struct QueryMetric {
    pub id: i32,
    pub uuid: String,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl QueryMetric {
    pub fn into_json(self) -> Result<JsonMetric, HttpError> {
        let Self {
            id: _,
            uuid: _,
            value,
            lower_bound,
            upper_bound,
        } = self;
        Ok(JsonMetric {
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = metric_table)]
pub struct InsertMetric {
    pub uuid: String,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl From<JsonMetric> for InsertMetric {
    fn from(latency: JsonMetric) -> Self {
        let JsonMetric {
            value,
            lower_bound,
            upper_bound,
        } = latency;
        Self {
            uuid: Uuid::new_v4().to_string(),
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        }
    }
}

impl InsertMetric {
    pub fn map_json(
        conn: &mut SqliteConnection,
        metric: Option<JsonMetric>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_metric) = metric {
            let insert_metric: InsertMetric = json_metric.into();
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(map_http_error!("Failed to create benchmark metric data."))?;

            Some(
                schema::metric::table
                    .filter(schema::metric::uuid.eq(&insert_metric.uuid))
                    .select(schema::metric::id)
                    .first::<i32>(conn)
                    .map_err(map_http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
