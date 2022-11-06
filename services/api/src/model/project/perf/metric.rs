use bencher_json::project::report::new::JsonMetrics;
use bencher_json::project::report::JsonMetric;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::metric as metric_table, util::map_http_error};

#[derive(Queryable, Debug)]
pub struct QueryMetric {
    pub id: i32,
    pub uuid: String,
    pub perf_id: i32,
    pub metric_kind_id: i32,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl QueryMetric {
    pub fn into_json(self) -> Result<JsonMetric, HttpError> {
        let Self {
            id: _,
            uuid: _,
            perf_id: _,
            metric_kind_id: _,
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
    pub perf_id: i32,
    pub metric_kind_id: i32,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl InsertMetric {
    fn from_json(perf_id: i32, metric_kind_id: i32, metric: JsonMetric) -> Self {
        let JsonMetric {
            value,
            lower_bound,
            upper_bound,
        } = metric;
        Self {
            perf_id,
            metric_kind_id,
            uuid: Uuid::new_v4().to_string(),
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        }
    }

    pub fn map_json(
        conn: &mut SqliteConnection,
        perf_id: i32,
        metric_kind_id: i32,
        metric: Option<JsonMetric>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_metric) = metric {
            let insert_metric: InsertMetric = Self::from_json(perf_id, metric_kind_id, metric);
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
