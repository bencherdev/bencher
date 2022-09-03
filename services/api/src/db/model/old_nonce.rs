use std::{
    cmp::Ordering,
    str::FromStr,
};

use bencher_json::auth::{
    JsonConfirmed,
    JsonNonce,
};
use chrono::Utc;
use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    project::QueryProject,
    user::QueryUser,
};
use crate::{
    db::{
        schema,
        schema::nonce as nonce_table,
    },
    diesel::{
        ExpressionMethods,
        JoinOnDsl,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

const NONCE_ERROR: &str = "Failed to get nonce.";

#[derive(Queryable)]
pub struct QueryNonce {
    pub id:       i32,
    pub uuid:     String,
    pub user_id:  i32,
    pub code:     i32,
    pub attempts: i32,
    pub creation: i64,
}

// 15 minutes * 60 seconds / minute
const NONCE_TTL: i64 = 15 * 60;
const CODE_SIZE: usize = 6;

impl QueryNonce {
    pub fn to_json(
        self,
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<JsonNonce, HttpError> {
        Ok(JsonNonce {
            email: QueryUser::get_email_from_id(conn, user_id)?,
            code:  self.code as u32,
        })
    }

    pub fn validate(
        conn: &mut SqliteConnection,
        json_nonce: &JsonNonce,
    ) -> Result<JsonConfirmed, HttpError> {
        let user_id = QueryUser::get_id_from_email(conn, &json_nonce.email)?;

        if json_nonce.code.to_string().len() != CODE_SIZE {
            return Err(http_error!(NONCE_ERROR));
        }

        let alive_nonce = AliveNonce::query(conn, user_id)?;

        if alive_nonce.code == json_nonce.code as i32 {
            diesel::update(schema::nonce::table.find(alive_nonce.id))
                .set(schema::nonce::attempts.eq(Attempts::Ok as i32 + alive_nonce.attempts))
                .execute(conn)
                .map_err(|_| http_error!(NONCE_ERROR))?;

            let user = schema::user::table
                .filter(schema::user::id.eq(user_id))
                .first::<QueryUser>(conn)
                .map_err(|_| http_error!(NONCE_ERROR))?
                .to_json()?;

            Ok(JsonConfirmed {
                user,
                // TODO jwt
                token: String::new(),
            })
        } else {
            diesel::update(schema::nonce::table.find(alive_nonce.id))
                .set(schema::nonce::attempts.eq(alive_nonce.attempts + 1))
                .execute(conn)
                .map_err(|_| http_error!(NONCE_ERROR))?;

            Err(http_error!(NONCE_ERROR))
        }
    }
}

const INIT: isize = 0;
const MAX: isize = 5;
const OK: isize = 20;
const OBE: isize = 40;

pub enum Attempts {
    Init = INIT,
    Max  = MAX,
    Ok   = OK,
    Obe  = OBE,
}

struct AliveNonce {
    pub id:       i32,
    pub code:     i32,
    pub attempts: i32,
}

impl AliveNonce {
    pub fn query(conn: &mut SqliteConnection, user_id: i32) -> Result<AliveNonce, HttpError> {
        schema::nonce::table
            .filter(schema::nonce::user_id.eq(user_id))
            .filter(schema::nonce::attempts.lt(Attempts::Max as i32))
            .filter(schema::nonce::creation.gt(Utc::now().timestamp() - NONCE_TTL))
            .select((
                schema::nonce::id,
                schema::nonce::code,
                schema::nonce::attempts,
            ))
            .first::<(i32, i32, i32)>(conn)
            .map(|(id, code, attempts)| Self { id, code, attempts })
            .map_err(|_| http_error!(NONCE_ERROR))
    }
}

#[derive(Insertable)]
#[diesel(table_name = nonce_table)]
pub struct InsertNonce {
    pub uuid:     String,
    pub user_id:  i32,
    pub code:     i32,
    pub attempts: i32,
    pub creation: i64,
}

impl InsertNonce {
    fn create(conn: &mut SqliteConnection, email: &str) -> Result<JsonNonce, HttpError> {
        let user_id = QueryUser::get_id_from_email(conn, email)?;

        if let Ok(nonce) = AliveNonce::query(conn, user_id) {
            diesel::update(schema::nonce::table.find(nonce.id))
                .set(schema::nonce::attempts.eq(Attempts::Obe as i32 + nonce.attempts))
                .execute(conn)
                .map_err(|_| http_error!(NONCE_ERROR))?;
        }

        let insert_nonce = Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            // TODO 6 digit code
            code: 0,
            attempts: Attempts::Init as i32,
            creation: Utc::now().timestamp(),
        };
        diesel::insert_into(schema::nonce::table)
            .values(&insert_nonce)
            .execute(conn)
            .map_err(|_| http_error!(NONCE_ERROR))?;
        let query_nonce = schema::nonce::table
            .filter(schema::nonce::uuid.eq(&insert_nonce.uuid))
            .first::<QueryNonce>(conn)
            .map_err(|_| http_error!(NONCE_ERROR))?;
        let json = query_nonce.to_json(conn, user_id)?;

        Ok(json)
    }
}
