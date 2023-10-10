use std::str::FromStr;

use bencher_json::{
    user::token::JsonUpdateToken, JsonNewToken, JsonToken, Jwt, NonEmpty, ResourceId, TokenUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::{DbConnection, Rbac, SecretKey},
    schema,
    schema::token as token_table,
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        to_date_time,
    },
    ApiError,
};

use super::{auth::AuthUser, QueryUser, UserId};

crate::util::typed_id::typed_id!(TokenId);

macro_rules! same_user {
    ($auth_user:ident, $rbac:expr, $user_id:expr) => {
        if !($auth_user.is_admin(&$rbac) || $auth_user.id == $user_id) {
            return Err(crate::error::ApiError::SameUser($auth_user.id, $user_id));
        }
    };
}

pub(crate) use same_user;

#[derive(diesel::Queryable)]
pub struct QueryToken {
    pub id: TokenId,
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: String,
    pub jwt: String,
    pub creation: i64,
    pub expiration: i64,
}

impl QueryToken {
    fn_get!(token, TokenId);
    fn_get_id!(token, TokenId, TokenUuid);
    fn_get_uuid!(token, TokenId, TokenUuid);

    pub fn get_user_token(
        conn: &mut DbConnection,
        user_id: UserId,
        uuid: &str,
    ) -> Result<Self, ApiError> {
        schema::token::table
            .filter(schema::token::user_id.eq(user_id))
            .filter(schema::token::uuid.eq(uuid))
            .first::<QueryToken>(conn)
            .map_err(ApiError::from)
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
            uuid,
            user: QueryUser::get_uuid(conn, user_id)?,
            name: NonEmpty::from_str(&name).map_err(ApiError::from)?,
            token: Jwt::from_str(&jwt).map_err(ApiError::from)?,
            creation: to_date_time(creation)?,
            expiration: to_date_time(expiration)?,
        })
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = token_table)]
pub struct InsertToken {
    pub uuid: TokenUuid,
    pub user_id: UserId,
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

        let claims = secret_key.validate_api_key(&jwt.as_ref().parse()?)?;

        Ok(Self {
            uuid: TokenUuid::new(),
            user_id: query_user.id,
            name: name.to_string(),
            jwt: jwt.to_string(),
            creation: claims.iat as i64,
            expiration: claims.exp as i64,
        })
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = token_table)]
pub struct UpdateToken {
    pub name: Option<String>,
}

impl From<JsonUpdateToken> for UpdateToken {
    fn from(update: JsonUpdateToken) -> Self {
        let JsonUpdateToken { name } = update;
        Self {
            name: name.map(Into::into),
        }
    }
}
