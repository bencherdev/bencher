use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewThreshold, JsonThreshold, JsonThresholdStatistic};
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl};
use uuid::Uuid;

use self::statistic::{InsertStatistic, QueryStatistic};
use super::{branch::QueryBranch, metric_kind::QueryMetricKind, testbed::QueryTestbed};
use crate::{
    context::DbConnection,
    error::api_error,
    schema::threshold as threshold_table,
    schema::{self},
    util::query::{fn_get, fn_get_id},
    ApiError,
};

pub mod alert;
pub mod boundary;
pub mod statistic;

#[derive(Queryable)]
pub struct QueryThreshold {
    pub id: i32,
    pub uuid: String,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub metric_kind_id: i32,
    pub statistic_id: i32,
}

impl QueryThreshold {
    fn_get!(threshold);
    fn_get_id!(threshold);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::threshold::table
            .filter(schema::threshold::id.eq(id))
            .select(schema::threshold::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, ApiError> {
        let Self {
            uuid,
            branch_id,
            testbed_id,
            metric_kind_id,
            statistic_id,
            ..
        } = self;
        Ok(JsonThreshold {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            metric_kind: QueryMetricKind::get(conn, metric_kind_id)?.into_json(conn)?,
            branch: QueryBranch::get(conn, branch_id)?.into_json(conn)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            statistic: QueryStatistic::get(conn, statistic_id)?.into_json()?,
        })
    }

    pub fn historical_json(
        conn: &mut DbConnection,
        threshold_id: i32,
        statistic_id: i32,
    ) -> Result<JsonThreshold, ApiError> {
        let mut threshold = Self::get(conn, threshold_id)?;
        // IMPORTANT: Set the statistic ID to the one from the boundary, and not the current value!
        threshold.statistic_id = statistic_id;
        threshold.into_json(conn)
    }

    pub fn threshold_statistic_json(
        conn: &mut DbConnection,
        metric_kind_id: i32,
        branch_id: i32,
        testbed_id: i32,
    ) -> Result<Option<JsonThresholdStatistic>, ApiError> {
        Ok(
            if let Ok(threshold) = schema::threshold::table
                .filter(schema::threshold::metric_kind_id.eq(metric_kind_id))
                .filter(schema::threshold::branch_id.eq(branch_id))
                .filter(schema::threshold::testbed_id.eq(testbed_id))
                .first::<QueryThreshold>(conn)
            {
                Some(JsonThresholdStatistic {
                    uuid: Uuid::from_str(&threshold.uuid).map_err(api_error!())?,
                    statistic: QueryStatistic::get(conn, threshold.statistic_id)?.into_json()?,
                })
            } else {
                None
            },
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: String,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub metric_kind_id: i32,
    pub statistic_id: i32,
}

impl InsertThreshold {
    pub fn new<U>(
        conn: &mut DbConnection,
        metric_kind_id: i32,
        branch_id: i32,
        testbed_id: i32,
        statistic: &U,
    ) -> Result<Self, ApiError>
    where
        U: ToString,
    {
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            branch_id,
            testbed_id,
            metric_kind_id,
            statistic_id: QueryStatistic::get_id(conn, statistic)?,
        })
    }

    pub fn from_json(
        conn: &mut DbConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        json_threshold: &JsonNewThreshold,
    ) -> Result<Self, ApiError> {
        let metric_kind_id =
            QueryMetricKind::from_resource_id(conn, project_id, &json_threshold.metric_kind)?.id;

        let insert_statistic = InsertStatistic::from_json(json_threshold.statistic)?;
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(api_error!())?;

        Self::new(
            conn,
            metric_kind_id,
            branch_id,
            testbed_id,
            &insert_statistic.uuid,
        )
    }
}
