use std::str::FromStr;

use bencher_json::{JsonNewToken, JsonToken, Jwt, NonEmpty, ResourceId};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::{DbConnection, Rbac, SecretKey},
    error::api_error,
    schema,
    schema::token as token_table,
    util::query::fn_get_id,
    ApiError,
};

use super::{auth::AuthUser, QueryUser};

macro_rules! same_user {
    ($auth_user:ident, $rbac:expr, $user_id:expr) => {
        if !($auth_user.is_admin(&$rbac) || $auth_user.id == $user_id) {
            return Err(crate::error::ApiError::SameUser($auth_user.id, $user_id));
        }
    };
}

pub(crate) use same_user;

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
    fn_get_id!(token);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::token::table
            .filter(schema::token::id.eq(id))
            .select(schema::token::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonToken, ApiError> {
        let Self {
            uuid,
            user_id,
            name,
            jwt,
            creation,
            expiration,
            ..
        } = self;
        Ok(JsonToken {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: QueryUser::get_uuid(conn, user_id)?,
            name: NonEmpty::from_str(&name).map_err(api_error!())?,
            token: Jwt::from_str(&jwt).map_err(api_error!())?,
            creation: to_date_time(creation)?,
            expiration: to_date_time(expiration)?,
        })
    }
}

pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, ApiError> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .ok_or(ApiError::Timestamp(timestamp))
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
    #[allow(clippy::cast_possible_wrap)]
    pub fn from_json(
        conn: &mut DbConnection,
        rbac: &Rbac,
        secret_key: &SecretKey,
        user: &ResourceId,
        token: JsonNewToken,
        auth_user: &AuthUser,
    ) -> Result<Self, ApiError> {
        let JsonNewToken { name, ttl } = token;

        let query_user = QueryUser::from_resource_id(conn, user)?;
        same_user!(auth_user, rbac, query_user.id);

        // TODO Custom max TTL
        let max_ttl = u32::MAX;
        let ttl = if let Some(ttl) = ttl {
            if ttl > max_ttl {
                return Err(ApiError::MaxTtl {
                    requested: ttl,
                    max: max_ttl,
                });
            }

            ttl
        } else {
            max_ttl
        };

        let jwt = secret_key.new_api_key(query_user.email.as_str().parse()?, ttl)?;

        let token_data = secret_key.validate_api_key(&jwt.as_ref().parse()?)?;

        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            user_id: query_user.id,
            name: name.to_string(),
            jwt: jwt.to_string(),
            creation: token_data.claims.iat as i64,
            expiration: token_data.claims.exp as i64,
        })
    }
}
