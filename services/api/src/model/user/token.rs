use bencher_json::{
    user::token::JsonUpdateToken, DateTime, JsonNewToken, JsonToken, Jwt, NonEmpty, ResourceId,
    TokenUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::{DbConnection, Rbac, SecretKey},
    schema,
    schema::token as token_table,
    util::query::{fn_get, fn_get_id, fn_get_uuid},
    ApiError,
};

use super::{auth::AuthUser, QueryUser, UserId};

crate::util::typed_id::typed_id!(TokenId);

macro_rules! same_user {
    ($auth_user:ident, $rbac:expr, $user_id:expr) => {
        if !($auth_user.is_admin(&$rbac) || $auth_user.id == $user_id) {
            return Err(crate::error::forbidden_error(format!("User is not admin and the authenticated user ({auth_user}) does not match the requested user ({requested_user})", auth_user = $auth_user.id, requested_user = $user_id))).map_err(Into::into);
        }
    };
}

pub(crate) use same_user;

#[derive(diesel::Queryable)]
pub struct QueryToken {
    pub id: TokenId,
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: NonEmpty,
    pub jwt: Jwt,
    pub creation: DateTime,
    pub expiration: DateTime,
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
            name,
            token: jwt,
            creation,
            expiration,
        })
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = token_table)]
pub struct InsertToken {
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: NonEmpty,
    pub jwt: Jwt,
    pub creation: DateTime,
    pub expiration: DateTime,
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

        let jwt = secret_key.new_api_key(query_user.email, ttl)?;

        let claims = secret_key.validate_api_key(&jwt.as_ref().parse()?)?;

        Ok(Self {
            uuid: TokenUuid::new(),
            user_id: query_user.id,
            name,
            jwt,
            creation: claims.issued_at(),
            expiration: claims.expiration(),
        })
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = token_table)]
pub struct UpdateToken {
    pub name: Option<NonEmpty>,
}

impl From<JsonUpdateToken> for UpdateToken {
    fn from(update: JsonUpdateToken) -> Self {
        let JsonUpdateToken { name } = update;
        Self { name }
    }
}
