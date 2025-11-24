use bencher_json::{
    DateTime, JsonNewToken, JsonToken, Jwt, ResourceName, TokenUuid, UserResourceId,
    user::token::JsonUpdateToken,
};
use bencher_token::TokenKey;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::{DbConnection, Rbac},
    error::{
        BencherResource, assert_parentage, bad_request_error, issue_error, resource_not_found_err,
    },
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    model::user::same_user,
    schema,
    schema::token as token_table,
};

use super::{QueryUser, UserId, auth::AuthUser};

crate::macros::typed_id::typed_id!(TokenId);

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryToken {
    pub id: TokenId,
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: ResourceName,
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
    ) -> Result<Self, HttpError> {
        schema::token::table
            .filter(schema::token::user_id.eq(user_id))
            .filter(schema::token::uuid.eq(uuid))
            .first::<QueryToken>(conn)
            .map_err(resource_not_found_err!(Token, (user_id, uuid)))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonToken, HttpError> {
        let query_user = QueryUser::get(conn, self.user_id)?;
        Ok(self.into_json_for_user(&query_user))
    }

    pub fn into_json_for_user(self, query_user: &QueryUser) -> JsonToken {
        let Self {
            uuid,
            user_id,
            name,
            jwt,
            creation,
            expiration,
            ..
        } = self;
        assert_parentage(
            BencherResource::User,
            query_user.id,
            BencherResource::Token,
            user_id,
        );
        JsonToken {
            uuid,
            user: query_user.uuid,
            name,
            token: jwt,
            creation,
            expiration,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = token_table)]
pub struct InsertToken {
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: ResourceName,
    pub jwt: Jwt,
    pub creation: DateTime,
    pub expiration: DateTime,
}

impl InsertToken {
    #[cfg(feature = "plus")]
    pub async fn rate_limit(
        context: &crate::ApiContext,
        query_user: &QueryUser,
    ) -> Result<(), HttpError> {
        use crate::{conn_lock, context::RateLimitingError};

        let resource = BencherResource::Token;
        let (start_time, end_time) = context.rate_limiting.window();
        let window_usage: u32 = schema::token::table
                .filter(schema::token::user_id.eq(query_user.id))
                .filter(schema::token::creation.ge(start_time))
                .filter(schema::token::creation.le(end_time))
                .count()
                .get_result::<i64>(conn_lock!(context))
                .map_err(resource_not_found_err!(Token, (query_user, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for user ({uuid}) between {start_time} and {end_time}.", uuid = query_user.uuid),
                    e
                    )}
                )?;

        context
            .rate_limiting
            .check_user_limit(window_usage, |rate_limit| RateLimitingError::User {
                user: query_user.clone(),
                resource,
                rate_limit,
            })
    }

    pub fn from_json(
        conn: &mut DbConnection,
        rbac: &Rbac,
        token_key: &TokenKey,
        user: &UserResourceId,
        token: JsonNewToken,
        auth_user: &AuthUser,
    ) -> Result<Self, HttpError> {
        let JsonNewToken { name, ttl } = token;

        let query_user = QueryUser::from_resource_id(conn, user)?;
        same_user!(auth_user, rbac, query_user.uuid);

        // TODO Custom max TTL
        let max_ttl = u32::MAX;
        let ttl = if let Some(ttl) = ttl {
            if ttl > max_ttl {
                return Err(bad_request_error(format!(
                    "Requested TTL ({ttl}) is greater than max ({max_ttl})"
                )));
            }
            ttl
        } else {
            max_ttl
        };

        let jwt = token_key.new_api_key(query_user.email, ttl).map_err(|e| {
            issue_error(
                "Failed to create new API key",
                "Failed to create new API key.",
                e,
            )
        })?;

        let claims = token_key.validate_api_key(&jwt).map_err(|e| {
            issue_error(
                "Failed to validate new API key",
                &format!("Failed to validate new API key: {jwt}"),
                e,
            )
        })?;

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
    pub name: Option<ResourceName>,
}

impl From<JsonUpdateToken> for UpdateToken {
    fn from(update: JsonUpdateToken) -> Self {
        let JsonUpdateToken { name } = update;
        Self { name }
    }
}
