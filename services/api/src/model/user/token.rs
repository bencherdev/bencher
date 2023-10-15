use bencher_json::{
    user::token::JsonUpdateToken, DateTime, JsonNewToken, JsonToken, Jwt, NonEmpty, ResourceId,
    TokenUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use crate::{
    context::{DbConnection, Rbac, SecretKey},
    error::{
        assert_parentage, bad_request_error, issue_error, resource_not_found_err, BencherResource,
    },
    model::user::same_user,
    schema,
    schema::token as token_table,
    util::query::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{auth::AuthUser, QueryUser, UserId};

crate::util::typed_id::typed_id!(TokenId);

#[derive(Debug, diesel::Queryable)]
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
    pub name: NonEmpty,
    pub jwt: Jwt,
    pub creation: DateTime,
    pub expiration: DateTime,
}

impl InsertToken {
    pub fn from_json(
        conn: &mut DbConnection,
        rbac: &Rbac,
        secret_key: &SecretKey,
        user: &ResourceId,
        token: JsonNewToken,
        auth_user: &AuthUser,
    ) -> Result<Self, HttpError> {
        let JsonNewToken { name, ttl } = token;

        let query_user = QueryUser::from_resource_id(conn, user)?;
        same_user!(auth_user, rbac, query_user.id);

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

        let jwt = secret_key.new_api_key(query_user.email, ttl).map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create new API key",
                "Failed to create new API key.",
                e,
            )
        })?;

        let claims = secret_key.validate_api_key(&jwt).map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
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
    pub name: Option<NonEmpty>,
}

impl From<JsonUpdateToken> for UpdateToken {
    fn from(update: JsonUpdateToken) -> Self {
        let JsonUpdateToken { name } = update;
        Self { name }
    }
}
