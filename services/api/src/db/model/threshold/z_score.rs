use std::str::FromStr;

use bencher_json::threshold::{
    JsonNewThreshold,
    JsonThreshold,
};
use diesel::{
    Insertable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::threshold as threshold_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

const Z_SCORE_ERROR: &str = "Failed to get Z-Score.";

#[derive(Queryable)]
pub struct QueryZScore {
    pub id: i32,
    pub uuid: String,
    pub sample_size: Option<i32>,
    pub min_deviation: Option<i32>,
    pub max_deviation: Option<i32>,
}

impl QueryZScore {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::z_score::table
            .filter(schema::z_score::uuid.eq(uuid.to_string()))
            .select(schema::z_score::id)
            .first(conn)
            .map_err(|_| http_error!(Z_SCORE_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::z_score::table
            .filter(schema::z_score::id.eq(id))
            .select(schema::z_score::uuid)
            .first(conn)
            .map_err(|_| http_error!(Z_SCORE_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(Z_SCORE_ERROR))
    }

    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonThreshold, HttpError> {
        let Self {
            id: _,
            uuid,
            sample_size,
            min_deviation,
            max_deviation,
        } = self;
        Ok(JsonThreshold {
            uuid:    Uuid::from_str(&uuid).map_err(|_| http_error!(THRESHOLD_ERROR))?,
            branch:  QueryBranch::get_uuid(conn, branch_id)?,
            testbed: QueryTestbed::get_uuid(conn, testbed_id)?,
            z_score: None,
            t_test:  None,
        })
    }
}

#[derive(Insertable)]
#[table_name = "threshold_table"]
pub struct InsertThreshold {
    pub uuid:       String,
    pub branch_id:  i32,
    pub testbed_id: i32,
}

impl InsertThreshold {
    pub fn from_json(
        conn: &SqliteConnection,
        json_threshold: JsonNewThreshold,
    ) -> Result<Self, HttpError> {
        let JsonNewThreshold { branch, testbed } = json_threshold;
        Ok(Self {
            uuid:       Uuid::new_v4().to_string(),
            branch_id:  QueryBranch::get_id(conn, &branch)?,
            testbed_id: QueryTestbed::get_id(conn, &testbed)?,
        })
    }
}
