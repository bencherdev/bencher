use std::str::FromStr;

use bencher_json::{
    project::threshold::{JsonNewStatistic, JsonStatistic, JsonStatisticKind},
    Boundary,
};
use chrono::Utc;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    schema,
    schema::statistic as statistic_table,
    util::{
        map_u32,
        query::{fn_get, fn_get_id},
        to_date_time,
    },
    ApiError,
};

#[derive(Queryable, Debug, Clone)]
pub struct QueryStatistic {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub test: i32,
    pub min_sample_size: Option<i64>,
    pub max_sample_size: Option<i64>,
    pub window: Option<i64>,
    pub lower_boundary: Option<f64>,
    pub upper_boundary: Option<f64>,
    pub created: i64,
}

impl QueryStatistic {
    fn_get!(statistic);
    fn_get_id!(statistic);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
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
            lower_boundary,
            upper_boundary,
            created,
            ..
        } = self;
        Ok(JsonStatistic {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            test: StatisticKind::try_from(test)?.into(),
            min_sample_size: map_u32(min_sample_size)?,
            max_sample_size: map_u32(max_sample_size)?,
            window: map_u32(window)?,
            lower_boundary: map_boundary(lower_boundary)?,
            upper_boundary: map_boundary(upper_boundary)?,
            created: to_date_time(created).map_err(api_error!())?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
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

pub fn map_boundary(boundary: Option<f64>) -> Result<Option<Boundary>, ApiError> {
    Ok(if let Some(boundary) = boundary {
        Some(boundary.try_into()?)
    } else {
        None
    })
}

#[derive(Insertable)]
#[diesel(table_name = statistic_table)]
pub struct InsertStatistic {
    pub uuid: String,
    pub project_id: i32,
    pub test: i32,
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
            project_id,
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
            uuid: Uuid::new_v4().to_string(),
            project_id,
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
    pub fn from_json(project_id: i32, json_statistic: JsonNewStatistic) -> Result<Self, ApiError> {
        let JsonNewStatistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = json_statistic;
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            test: StatisticKind::from(test) as i32,
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
            created: Utc::now().timestamp(),
        })
    }
}
