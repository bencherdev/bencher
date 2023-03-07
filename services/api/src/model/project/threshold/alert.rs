use std::str::FromStr;

use bencher_json::project::alert::{JsonAlert, JsonSide};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::{statistic::QueryStatistic, QueryThreshold};
use crate::{
    context::DbConnection, error::api_error, model::project::perf::QueryPerf, schema,
    schema::alert as alert_table, util::query::fn_get_id, ApiError,
};

#[derive(Queryable)]
pub struct QueryAlert {
    pub id: i32,
    pub uuid: String,
    pub perf_id: i32,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side: bool,
    pub boundary: f32,
    pub outlier: f32,
}

impl QueryAlert {
    fn_get_id!(alert);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::alert::table
            .filter(schema::alert::id.eq(id))
            .select(schema::alert::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonAlert, ApiError> {
        let Self {
            uuid,
            perf_id,
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
            ..
        } = self;
        Ok(JsonAlert {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            perf: QueryPerf::get_uuid(conn, perf_id)?,
            threshold: QueryThreshold::get_uuid(conn, threshold_id)?,
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
            side: Side::from(side).into(),
            boundary: boundary.into(),
            outlier: outlier.into(),
        })
    }
}

pub enum Side {
    Left = 0,
    Right = 1,
}

impl From<bool> for Side {
    fn from(side: bool) -> Self {
        if side {
            Self::Right
        } else {
            Self::Left
        }
    }
}

impl From<Side> for bool {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => false,
            Side::Right => true,
        }
    }
}

impl From<Side> for JsonSide {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: String,
    pub perf_id: i32,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side: bool,
    pub boundary: f32,
    pub outlier: f32,
}
