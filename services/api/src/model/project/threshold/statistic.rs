use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewStatistic, JsonStatistic, JsonStatisticKind};
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use crate::{
    error::api_error, schema, schema::statistic as statistic_table, util::query::fn_get_id,
    ApiError,
};

#[derive(Queryable, Debug, Clone)]
pub struct QueryStatistic {
    pub id: i32,
    pub uuid: String,
    pub test: i32,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub left_side: Option<f32>,
    pub right_side: Option<f32>,
}

impl QueryStatistic {
    fn_get_id!(statistic);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::statistic::table
            .filter(schema::statistic::id.eq(id))
            .select(schema::statistic::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn into_json(self) -> Result<JsonStatistic, ApiError> {
        let Self {
            uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            left_side,
            right_side,
            ..
        } = self;
        Ok(JsonStatistic {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            test: StatisticKind::try_from(test)?.into(),
            min_sample_size: min_sample_size.map(|ss| ss as u32),
            max_sample_size: max_sample_size.map(|ss| ss as u32),
            window: window.map(|w| w as u32),
            left_side: left_side.map(Into::into),
            right_side: right_side.map(Into::into),
        })
    }
}

#[derive(Clone, Copy)]
pub enum StatisticKind {
    Z = 0,
    T = 1,
}

impl TryFrom<i32> for StatisticKind {
    type Error = ApiError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        match kind {
            0 => Ok(Self::Z),
            1 => Ok(Self::T),
            _ => Err(ApiError::StatisticKind(kind)),
        }
    }
}

impl From<JsonStatisticKind> for StatisticKind {
    fn from(kind: JsonStatisticKind) -> Self {
        match kind {
            JsonStatisticKind::Z => Self::Z,
            JsonStatisticKind::T => Self::T,
        }
    }
}

impl From<StatisticKind> for JsonStatisticKind {
    fn from(kind: StatisticKind) -> Self {
        match kind {
            StatisticKind::Z => Self::Z,
            StatisticKind::T => Self::T,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = statistic_table)]
pub struct InsertStatistic {
    pub uuid: String,
    pub test: i32,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub left_side: Option<f32>,
    pub right_side: Option<f32>,
}

impl InsertStatistic {
    pub fn from_json(json_statistic: JsonNewStatistic) -> Result<Self, ApiError> {
        let JsonNewStatistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            left_side,
            right_side,
        } = json_statistic;
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            test: StatisticKind::from(test) as i32,
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window: window.map(Into::into),
            left_side: left_side.map(Into::into),
            right_side: right_side.map(Into::into),
        })
    }
}
