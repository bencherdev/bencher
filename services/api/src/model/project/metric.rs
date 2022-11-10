use bencher_json::JsonMetric;
use diesel::Insertable;
use uuid::Uuid;

use crate::schema::metric as metric_table;

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
    pub fn into_json(self) -> JsonMetric {
        let Self {
            id: _,
            uuid: _,
            perf_id: _,
            metric_kind_id: _,
            value,
            lower_bound,
            upper_bound,
        } = self;
        JsonMetric {
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        }
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
    pub fn from_json(perf_id: i32, metric_kind_id: i32, metric: JsonMetric) -> Self {
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
}
