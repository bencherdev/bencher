use bencher_json::{
    project::threshold::{JsonNewStatistic, JsonStatistic, StatisticKind},
    Boundary, SampleSize, StatisticUuid, ThresholdUuid,
};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::DbConnection,
    schema,
    schema::statistic as statistic_table,
    util::{
        map_u32,
        query::{fn_get, fn_get_id, fn_get_uuid},
        to_date_time,
    },
    ApiError,
};

use super::{QueryThreshold, ThresholdId};

crate::util::typed_id::typed_id!(StatisticId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = statistic_table)]
pub struct QueryStatistic {
    pub id: StatisticId,
    pub uuid: StatisticUuid,
    pub threshold_id: ThresholdId,
    pub test: StatisticKind,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub lower_boundary: Option<f64>,
    pub upper_boundary: Option<f64>,
    pub created: i64,
}

impl QueryStatistic {
    fn_get!(statistic, StatisticId);
    fn_get_id!(statistic, StatisticId, StatisticUuid);
    fn_get_uuid!(statistic, StatisticId, StatisticUuid);

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonStatistic, ApiError> {
        let threshold = QueryThreshold::get_uuid(conn, self.threshold_id)?;
        self.into_json_for_threshold(threshold)
    }

    pub fn into_json_for_threshold(
        self,
        threshold: ThresholdUuid,
    ) -> Result<JsonStatistic, ApiError> {
        let Self {
            uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            ..
        } = self;
        Ok(JsonStatistic {
            uuid,
            threshold,
            test,
            min_sample_size: map_sample_size(min_sample_size)?,
            max_sample_size: map_sample_size(max_sample_size)?,
            window: map_u32(window)?,
            lower_boundary: map_boundary(lower_boundary)?,
            upper_boundary: map_boundary(upper_boundary)?,
            created: to_date_time(created).map_err(ApiError::from)?,
        })
    }
}

pub fn map_sample_size(sample_size: Option<i64>) -> Result<Option<SampleSize>, ApiError> {
    Ok(if let Some(sample_size) = sample_size {
        Some(u32::try_from(sample_size)?.try_into()?)
    } else {
        None
    })
}

pub fn map_boundary(boundary: Option<f64>) -> Result<Option<Boundary>, ApiError> {
    Ok(if let Some(boundary) = boundary {
        Some(boundary.try_into()?)
    } else {
        None
    })
}

#[derive(diesel::Insertable)]
#[diesel(table_name = statistic_table)]
pub struct InsertStatistic {
    pub uuid: StatisticUuid,
    pub threshold_id: ThresholdId,
    pub test: StatisticKind,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub lower_boundary: Option<f64>,
    pub upper_boundary: Option<f64>,
    pub created: i64,
}

impl From<QueryStatistic> for InsertStatistic {
    fn from(query_statistic: QueryStatistic) -> Self {
        let QueryStatistic {
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            ..
        } = query_statistic;
        Self {
            uuid: StatisticUuid::new(),
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
        }
    }
}

impl InsertStatistic {
    pub fn from_json(
        threshold_id: ThresholdId,
        json_statistic: JsonNewStatistic,
    ) -> Result<Self, ApiError> {
        let JsonNewStatistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = json_statistic;
        Ok(Self {
            uuid: StatisticUuid::new(),
            threshold_id,
            test,
            min_sample_size: min_sample_size.map(|ss| u32::from(ss).into()),
            max_sample_size: max_sample_size.map(|ss| u32::from(ss).into()),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
            created: Utc::now().timestamp(),
        })
    }
}
