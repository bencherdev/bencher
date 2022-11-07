use std::str::FromStr;

use bencher_json::project::threshold::{JsonNewThreshold, JsonThreshold};
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use self::statistic::{InsertStatistic, QueryStatistic};
use super::{branch::QueryBranch, testbed::QueryTestbed};
use crate::{
    error::api_error, schema, schema::threshold as threshold_table, util::http_error, ApiError,
};

pub mod alert;
pub mod statistic;

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
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, ApiError> {
        schema::threshold::table
            .filter(schema::threshold::uuid.eq(uuid.to_string()))
            .select(schema::threshold::id)
            .first(conn)
            .map_err(api_error!())
    }

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
            id: _,
            uuid,
            branch_id,
            testbed_id,
            kind,
            statistic_id,
        } = self;
        Ok(JsonThreshold {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
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
            _ => Err(http_error!("Failed to get threshold.")),
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

impl From<PerfKind> for JsonPerfKind {
    fn from(kind: PerfKind) -> Self {
        match kind {
            PerfKind::Throughput => Self::Throughput,
            PerfKind::Latency => Self::Latency,
            PerfKind::Compute => Self::Compute,
            PerfKind::Memory => Self::Memory,
            PerfKind::Storage => Self::Storage,
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
        branch_id: i32,
        testbed_id: i32,
        json_threshold: JsonNewThreshold,
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
            kind: PerfKind::from(json_threshold.kind) as i32,
            statistic_id: QueryStatistic::get_id(conn, &insert_statistic.uuid)?,
        })
    }
}
