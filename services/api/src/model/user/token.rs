use std::str::FromStr;

use bencher_json::{jwt::JsonWebToken, JsonNewToken, JsonToken};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::token as token_table, util::http_error};

use super::QueryUser;

const TOKEN_ERROR: &str = "Failed to get token.";

// ~365 days / years * 24 hours / day * 60 minutes / hour * 60 seconds / minute
#[cfg(not(debug_assert))]
const MAX_TTL: u64 = 365 * 24 * 60 * 60;

#[derive(Queryable)]
pub struct QueryToken {
    pub id: i32,
    pub uuid: String,
    pub user_id: i32,
    pub name: String,
    pub jwt: String,
    pub creation: i64,
    pub expiration: i64,
}

impl QueryToken {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::token::table
            .filter(schema::token::uuid.eq(uuid.to_string()))
            .select(schema::token::id)
            .first(conn)
            .map_err(|_| http_error!(TOKEN_ERROR))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::token::table
            .filter(schema::token::id.eq(id))
            .select(schema::token::uuid)
            .first(conn)
            .map_err(|_| http_error!(TOKEN_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(TOKEN_ERROR))
    }

    pub fn to_json(self, conn: &mut SqliteConnection) -> Result<JsonToken, HttpError> {
        let Self {
            id: _,
            uuid,
            user_id,
            name,
            jwt,
            creation,
            expiration,
        } = self;
        Ok(JsonToken {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(TOKEN_ERROR))?,
            user: QueryUser::get_uuid(conn, user_id)?,
            name,
            token: jwt,
            creation: to_date_time(creation)?,
            expiration: to_date_time(expiration)?,
        })
    }
}

pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, HttpError> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .ok_or(http_error!(TOKEN_ERROR))
}

#[derive(Insertable)]
#[diesel(table_name = token_table)]
pub struct InsertToken {
    pub uuid: String,
    pub user_id: i32,
    pub name: String,
    pub jwt: String,
    pub creation: i64,
    pub expiration: i64,
}

impl InsertToken {
    pub fn from_json(
        conn: &mut SqliteConnection,
        token: JsonNewToken,
        requester_id: i32,
        key: &str,
    ) -> Result<Self, HttpError> {
        let JsonNewToken { user, name, ttl } = token;

        let query_user = QueryUser::from_resource_id(conn, &user)?;

        // TODO make smarter once permissions are a thing
        if query_user.id != requester_id {
            return Err(http_error!(TOKEN_ERROR));
        }

        // Only in production, set a max TTL of approximately one year
        // Disabled in development to enable long lived testing tokens
        #[cfg(not(debug_assert))]
        if ttl > MAX_TTL {
            return Err(http_error!(TOKEN_ERROR));
        }

        let jwt = JsonWebToken::new_api_key(key, query_user.email, ttl as usize)
            .map_err(|_| http_error!(TOKEN_ERROR))?;

        let token_data = jwt
            .validate_api_key(key)
            .map_err(|_| http_error!(TOKEN_ERROR))?;

        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            user_id: query_user.id,
            name,
            jwt: jwt.to_string(),
            creation: token_data.claims.iat as i64,
            expiration: token_data.claims.exp as i64,
        })
    }
}
