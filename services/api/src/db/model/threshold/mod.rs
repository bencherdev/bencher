use std::str::FromStr;

use bencher_json::{
    perf::JsonPerfKind,
    threshold::{JsonNewThreshold, JsonThreshold},
};
use diesel::{Insertable, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use self::statistic::{InsertStatistic, QueryStatistic};
use super::{branch::QueryBranch, testbed::QueryTestbed};
use crate::{
    db::{schema, schema::threshold as threshold_table},
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    util::http_error,
};

pub mod alert;
pub mod statistic;

const THRESHOLD_ERROR: &str = "Failed to get threshold.";

#[derive(Queryable)]
pub struct QueryThreshold {
    pub id: i32,
    pub uuid: String,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub kind: i32,
    pub statistic_id: i32,
}

impl QueryThreshold {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::threshold::table
            .filter(schema::threshold::uuid.eq(uuid.to_string()))
            .select(schema::threshold::id)
            .first(conn)
            .map_err(|_| http_error!(THRESHOLD_ERROR))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::threshold::table
            .filter(schema::threshold::id.eq(id))
            .select(schema::threshold::uuid)
            .first(conn)
            .map_err(|_| http_error!(THRESHOLD_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(THRESHOLD_ERROR))
    }

    pub fn to_json(self, conn: &mut SqliteConnection) -> Result<JsonThreshold, HttpError> {
        let Self {
            id: _,
            uuid,
            branch_id,
            testbed_id,
            kind,
            statistic_id,
        } = self;
        Ok(JsonThreshold {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(THRESHOLD_ERROR))?,
            branch: QueryBranch::get_uuid(conn, branch_id)?,
            testbed: QueryTestbed::get_uuid(conn, testbed_id)?,
            kind: PerfKind::try_from(kind)?.into(),
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
        })
    }
}

#[derive(Copy, Clone)]
pub enum PerfKind {
    Throughput = 0,
    Latency = 1,
    Compute = 2,
    Memory = 3,
    Storage = 4,
}

impl TryFrom<i32> for PerfKind {
    type Error = HttpError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        match kind {
            0 => Ok(Self::Throughput),
            1 => Ok(Self::Latency),
            2 => Ok(Self::Compute),
            3 => Ok(Self::Memory),
            4 => Ok(Self::Storage),
            _ => Err(http_error!(THRESHOLD_ERROR)),
        }
    }
}

impl From<JsonPerfKind> for PerfKind {
    fn from(kind: JsonPerfKind) -> Self {
        match kind {
            JsonPerfKind::Throughput => Self::Throughput,
            JsonPerfKind::Latency => Self::Latency,
            JsonPerfKind::Compute => Self::Compute,
            JsonPerfKind::Memory => Self::Memory,
            JsonPerfKind::Storage => Self::Storage,
        }
    }
}

impl Into<JsonPerfKind> for PerfKind {
    fn into(self) -> JsonPerfKind {
        match self {
            Self::Throughput => JsonPerfKind::Throughput,
            Self::Latency => JsonPerfKind::Latency,
            Self::Compute => JsonPerfKind::Compute,
            Self::Memory => JsonPerfKind::Memory,
            Self::Storage => JsonPerfKind::Storage,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: String,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub kind: i32,
    pub statistic_id: i32,
}

impl InsertThreshold {
    pub fn from_json(
        conn: &mut SqliteConnection,
        json_threshold: JsonNewThreshold,
    ) -> Result<Self, HttpError> {
        let JsonNewThreshold {
            branch,
            testbed,
            kind,
            statistic,
        } = json_threshold;

        let insert_statistic = InsertStatistic::from_json(statistic)?;
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(|_| http_error!(THRESHOLD_ERROR))?;

        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            branch_id: QueryBranch::get_id(conn, &branch)?,
            testbed_id: QueryTestbed::get_id(conn, &testbed)?,
            kind: PerfKind::from(kind) as i32,
            statistic_id: QueryStatistic::get_id(conn, &insert_statistic.uuid)?,
        })
    }
}
