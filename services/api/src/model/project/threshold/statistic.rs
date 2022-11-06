use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewStatistic, JsonStatistic, JsonStatisticKind};
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    schema,
    schema::statistic as statistic_table,
    util::{http_error, map_http_error},
};

#[derive(Queryable)]
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
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::statistic::table
            .filter(schema::statistic::uuid.eq(uuid.to_string()))
            .select(schema::statistic::id)
            .first(conn)
            .map_err(map_http_error!("Failed to get statistic."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::statistic::table
            .filter(schema::statistic::id.eq(id))
            .select(schema::statistic::uuid)
            .first(conn)
            .map_err(map_http_error!("Failed to get statistic."))?;
        Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get statistic."))
    }

    pub fn into_json(self) -> Result<JsonStatistic, HttpError> {
        let Self {
            id: _,
            uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            left_side,
            right_side,
        } = self;
        Ok(JsonStatistic {
            uuid: Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get statistic."))?,
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
    type Error = HttpError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        match kind {
            0 => Ok(Self::Z),
            1 => Ok(Self::T),
            _ => Err(http_error!("Failed to get statistic.")),
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
    pub fn from_json(json_statistic: JsonNewStatistic) -> Result<Self, HttpError> {
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
            min_sample_size: min_sample_size.map(|ss| ss as i64),
            max_sample_size: max_sample_size.map(|ss| ss as i64),
            window: window.map(|w| w as i64),
            left_side: left_side.map(Into::into),
            right_side: right_side.map(Into::into),
        })
    }
}
