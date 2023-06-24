use std::str::FromStr;

use bencher_json::project::alert::{JsonAlert, JsonAlertStatus, JsonSide};
use chrono::{TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::boundary::QueryBoundary;
use crate::{
    context::DbConnection, error::api_error, schema, schema::alert as alert_table,
    util::query::fn_get_id, ApiError,
};

#[derive(Queryable)]
pub struct QueryAlert {
    pub id: i32,
    pub uuid: String,
    pub boundary_id: i32,
    pub side: bool,
    pub status: i32,
    pub modified: i64,
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
            boundary_id,
            side,
            status,
            modified,
            ..
        } = self;

        Ok(JsonAlert {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            boundary: QueryBoundary::get(conn, boundary_id)?.into_json(conn)?,
            side: Side::from(side).into(),
            status: Status::try_from(status)?.into(),
            modified: Utc
                .timestamp_opt(modified, 0)
                .single()
                .ok_or(ApiError::Timestamp(modified))?,
        })
    }
}

#[derive(Debug, PartialEq)]
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

#[derive(Default)]
pub enum Status {
    #[default]
    Unread = 0,
    Read = 1,
}

impl TryFrom<i32> for Status {
    type Error = ApiError;

    fn try_from(status: i32) -> Result<Self, Self::Error> {
        match status {
            0 => Ok(Self::Unread),
            1 => Ok(Self::Read),
            _ => Err(ApiError::BadAlertStatus(status)),
        }
    }
}

impl From<Status> for i32 {
    fn from(status: Status) -> Self {
        match status {
            Status::Unread => 0,
            Status::Read => 1,
        }
    }
}

impl From<Status> for JsonAlertStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Unread => Self::Unread,
            Status::Read => Self::Read,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: String,
    pub boundary_id: i32,
    pub side: bool,
    pub status: i32,
    pub modified: i64,
}

impl InsertAlert {
    pub fn from_boundary(
        conn: &mut DbConnection,
        boundary: Uuid,
        side: Side,
    ) -> Result<(), ApiError> {
        let insert_alert = InsertAlert {
            uuid: Uuid::new_v4().to_string(),
            boundary_id: QueryBoundary::get_id(conn, &boundary)?,
            side: side.into(),
            status: Status::default().into(),
            modified: Utc::now().timestamp(),
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(api_error!())?;

        Ok(())
    }
}
