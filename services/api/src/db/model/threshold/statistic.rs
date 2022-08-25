use std::str::FromStr;

use bencher_json::threshold::{
    JsonNewStatistic,
    JsonStatistic,
    JsonStatisticKind,
};
use chrono::offset::Utc;
use diesel::{
    expression_methods::BoolExpressionMethods,
    Insertable,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::statistic as statistic_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const STATISTIC_ERROR: &str = "Failed to get statistic.";

#[derive(Queryable)]
pub struct QueryStatistic {
    pub id:          i32,
    pub uuid:        String,
    pub kind:        i32,
    pub sample_size: Option<i64>,
    pub window:      Option<i64>,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

impl QueryStatistic {
    pub fn get_id(conn: &SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::statistic::table
            .filter(schema::statistic::uuid.eq(uuid.to_string()))
            .select(schema::statistic::id)
            .first(conn)
            .map_err(|_| http_error!(STATISTIC_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::statistic::table
            .filter(schema::statistic::id.eq(id))
            .select(schema::statistic::uuid)
            .first(conn)
            .map_err(|_| http_error!(STATISTIC_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(STATISTIC_ERROR))
    }

    pub fn to_json(self) -> Result<JsonStatistic, HttpError> {
        let Self {
            id: _,
            uuid,
            kind,
            sample_size,
            window,
            left_side,
            right_side,
        } = self;
        Ok(JsonStatistic {
            uuid:        Uuid::from_str(&uuid).map_err(|_| http_error!(STATISTIC_ERROR))?,
            kind:        StatisticKind::try_from(kind)?.into(),
            sample_size: sample_size.map(|ss| ss as u32),
            window:      window.map(|w| w as u32),
            left_side:   left_side.map(Into::into),
            right_side:  right_side.map(Into::into),
        })
    }
}

pub enum StatisticKind {
    Z = 0,
    T = 1,
}

impl TryFrom<i32> for StatisticKind {
    type Error = HttpError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        match kind {
            0 => Ok(StatisticKind::Z),
            1 => Ok(StatisticKind::T),
            _ => Err(http_error!(STATISTIC_ERROR)),
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

impl Into<JsonStatisticKind> for StatisticKind {
    fn into(self) -> JsonStatisticKind {
        match self {
            Self::Z => JsonStatisticKind::Z,
            Self::T => JsonStatisticKind::T,
        }
    }
}

pub struct ThresholdStatistic {
    pub branch_id:    i32,
    pub testbed_id:   i32,
    pub threshold_id: i32,
    pub statistic:    Statistic,
}

pub struct Statistic {
    pub id:          i32,
    pub uuid:        String,
    pub kind:        StatisticKind,
    pub sample_size: i64,
    pub window:      i64,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

impl ThresholdStatistic {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
    ) -> Result<Self, HttpError> {
        let threshold_statistic = schema::statistic::table
            .inner_join(
                schema::threshold::table
                    .on(schema::statistic::id.eq(schema::threshold::statistic_id)),
            )
            .filter(
                schema::threshold::branch_id
                    .eq(branch_id)
                    .and(schema::threshold::testbed_id.eq(testbed_id)),
            )
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::kind,
                schema::statistic::sample_size,
                schema::statistic::window,
                schema::statistic::left_side,
                schema::statistic::right_side,
            ))
            .first::<(
                i32,
                i32,
                String,
                i32,
                Option<i64>,
                Option<i64>,
                Option<f32>,
                Option<f32>,
            )>(conn)
            .map(
                |(threshold_id, id, uuid, kind, sample_size, window, left_side, right_side)| -> Result<ThresholdStatistic, HttpError> {
                    let statistic = Statistic {
                        id,
                        uuid,
                        kind: kind.try_into()?,
                        sample_size: unwrap_sample_size(sample_size),
                        window: unwrap_window(window),
                        left_side,
                        right_side,
                    };
                    Ok(Self {
                        branch_id,
                        testbed_id,
                        threshold_id,
                        statistic,
                    })
                },
            )
            .map_err(|_| http_error!(STATISTIC_ERROR))??;

        Ok(threshold_statistic)
    }
}

fn unwrap_sample_size(sample_size: Option<i64>) -> i64 {
    sample_size.unwrap_or(i64::MAX)
}

fn unwrap_window(window: Option<i64>) -> i64 {
    window
        .map(|window| {
            let now = Utc::now().timestamp_nanos();
            now - window
        })
        .unwrap_or_default()
}

#[derive(Insertable)]
#[table_name = "statistic_table"]
pub struct InsertStatistic {
    pub uuid:        String,
    pub kind:        i32,
    pub sample_size: Option<i64>,
    pub window:      Option<i64>,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

impl InsertStatistic {
    pub fn from_json(json_statistic: JsonNewStatistic) -> Result<Self, HttpError> {
        let JsonNewStatistic {
            kind,
            sample_size,
            window,
            left_side,
            right_side,
        } = json_statistic;
        Ok(Self {
            uuid:        Uuid::new_v4().to_string(),
            kind:        StatisticKind::from(kind) as i32,
            sample_size: sample_size.map(|ss| ss as i64),
            window:      window.map(|w| w as i64),
            left_side:   left_side.map(Into::into),
            right_side:  right_side.map(Into::into),
        })
    }
}
