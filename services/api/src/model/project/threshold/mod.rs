use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewThreshold, JsonThreshold};
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use self::statistic::{InsertStatistic, QueryStatistic};
use super::{branch::QueryBranch, metric_kind::QueryMetricKind, testbed::QueryTestbed};
use crate::{
    error::api_error, schema, schema::threshold as threshold_table, util::query::fn_get_id,
    ApiError,
};

pub mod alert;
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
    fn_get_id!(threshold);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::threshold::table
            .filter(schema::threshold::id.eq(id))
            .select(schema::threshold::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonThreshold, ApiError> {
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
            branch: QueryBranch::get_uuid(conn, branch_id)?,
            testbed: QueryTestbed::get_uuid(conn, testbed_id)?,
            metric_kind: QueryMetricKind::get_uuid(conn, metric_kind_id)?,
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
        })
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
    pub fn from_json(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        json_threshold: &JsonNewThreshold,
    ) -> Result<Self, ApiError> {
        let insert_statistic = InsertStatistic::from_json(json_threshold.statistic)?;
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(api_error!())?;

        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            branch_id,
            testbed_id,
            metric_kind_id: QueryMetricKind::from_resource_id(
                conn,
                project_id,
                &json_threshold.metric_kind,
            )?
            .id,
            statistic_id: QueryStatistic::get_id(conn, &insert_statistic.uuid)?,
        })
    }
}
