use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewThreshold, JsonThreshold, JsonThresholdStatistic};
use chrono::Utc;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl};
use uuid::Uuid;

use self::statistic::{InsertStatistic, QueryStatistic};
use super::{
    branch::QueryBranch, metric_kind::QueryMetricKind, testbed::QueryTestbed, QueryProject,
};
use crate::{
    context::DbConnection,
    error::api_error,
    schema::threshold as threshold_table,
    schema::{self},
    util::{
        query::{fn_get, fn_get_id},
        to_date_time,
    },
    ApiError,
};

pub mod alert;
pub mod boundary;
pub mod statistic;

#[derive(Queryable)]
pub struct QueryThreshold {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub metric_kind_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub statistic_id: i32,
    pub created: i64,
    pub modified: i64,
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

    pub fn get_with_statistic(
        conn: &mut DbConnection,
        threshold_id: i32,
        statistic_id: i32,
    ) -> Result<Self, ApiError> {
        let mut threshold = Self::get(conn, threshold_id)?;
        // IMPORTANT: Set the statistic ID to the one specified and not the current value!
        threshold.statistic_id = statistic_id;
        Ok(threshold)
    }

    pub fn get_json(
        conn: &mut DbConnection,
        threshold_id: i32,
        statistic_id: i32,
    ) -> Result<JsonThreshold, ApiError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?.into_json(conn)
    }

    pub fn get_threshold_statistic_json(
        conn: &mut DbConnection,
        threshold_id: i32,
        statistic_id: i32,
    ) -> Result<JsonThresholdStatistic, ApiError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?
            .into_threshold_statistic_json(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, ApiError> {
        let Self {
            uuid,
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            statistic_id,
            created,
            modified,
            ..
        } = self;
        Ok(JsonThreshold {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            metric_kind: QueryMetricKind::get(conn, metric_kind_id)?.into_json(conn)?,
            branch: QueryBranch::get(conn, branch_id)?.into_json(conn)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            statistic: QueryStatistic::get(conn, statistic_id)?.into_json(conn)?,
            created: to_date_time(created).map_err(api_error!())?,
            modified: to_date_time(modified).map_err(api_error!())?,
        })
    }

    pub fn into_threshold_statistic_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonThresholdStatistic, ApiError> {
        let Self {
            uuid,
            project_id,
            statistic_id,
            created,
            ..
        } = self;
        Ok(JsonThresholdStatistic {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            statistic: QueryStatistic::get(conn, statistic_id)?.into_json(conn)?,
            created: to_date_time(created).map_err(api_error!())?,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: String,
    pub project_id: i32,
    pub metric_kind_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub statistic_id: i32,
    pub created: i64,
    pub modified: i64,
}

impl InsertThreshold {
    pub fn new<U>(
        conn: &mut DbConnection,
        project_id: i32,
        metric_kind_id: i32,
        branch_id: i32,
        testbed_id: i32,
        statistic: &U,
    ) -> Result<Self, ApiError>
    where
        U: ToString,
    {
        let timestamp = Utc::now().timestamp();
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            statistic_id: QueryStatistic::get_id(conn, statistic)?,
            created: timestamp,
            modified: timestamp,
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

        let insert_statistic = InsertStatistic::from_json(project_id, json_threshold.statistic)?;
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(api_error!())?;

        Self::new(
            conn,
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            &insert_statistic.uuid,
        )
    }
}
