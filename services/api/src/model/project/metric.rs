use bencher_json::JsonMetric;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    schema::{self, metric as metric_table},
    ApiError,
};

use super::metric_kind::MetricKindId;

crate::util::typed_id::typed_id!(MetricId);

#[derive(Queryable, Debug)]
pub struct QueryMetric {
    pub id: MetricId,
    pub uuid: String,
    pub perf_id: i32,
    pub metric_kind_id: MetricKindId,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl QueryMetric {
    pub fn from_uuid(conn: &mut DbConnection, uuid: String) -> Result<Self, ApiError> {
        schema::metric::table
            .filter(schema::metric::uuid.eq(uuid))
            .first::<Self>(conn)
            .map_err(api_error!())
    }

    pub fn json(value: f64, lower_bound: Option<f64>, upper_bound: Option<f64>) -> JsonMetric {
        JsonMetric {
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        }
    }

    pub fn into_json(self) -> JsonMetric {
        let Self {
            value,
            lower_bound,
            upper_bound,
            ..
        } = self;
        Self::json(value, lower_bound, upper_bound)
    }
}

#[derive(Insertable)]
#[diesel(table_name = metric_table)]
pub struct InsertMetric {
    pub uuid: String,
    pub perf_id: i32,
    pub metric_kind_id: MetricKindId,
    pub value: f64,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl InsertMetric {
    pub fn from_json(perf_id: i32, metric_kind_id: MetricKindId, metric: JsonMetric) -> Self {
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
