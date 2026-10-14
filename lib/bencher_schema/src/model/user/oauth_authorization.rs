use bencher_json::{
    DateTime, JsonOAuthAuthorization, JsonOAuthAuthorizationClient, JsonOAuthAuthorizationRedirect,
    NonEmpty, OAuthAuthorizationUuid, OAuthClientId, OAuthCodeChallenge, OAuthCodeHash,
    OAuthGrantUuid, OAuthNonce, OAuthRedirectUri, OAuthRequestState, OAuthResource, OAuthScopes,
};
use chrono::TimeDelta;
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, OptionalExtension as _,
    QueryDsl as _, RunQueryDsl as _,
};

use crate::{
    context::{DbConnection, OAuthServer},
    schema,
};

use super::{
    QueryUser, UserId,
    key::{QueryUserKey, UserKeyId},
    oauth::{OAuthGrantId, QueryOAuthGrant, earliest},
};

crate::macros::typed_id::typed_id!(OAuthAuthorizationId);

/// How long a user has to answer an app's request to connect.
const PENDING: TimeDelta = TimeDelta::minutes(30);
/// How long an app has to exchange its authorization code.
const CODE: TimeDelta = TimeDelta::minutes(5);
/// How long a grant lasts at most.
const GRANT: TimeDelta = TimeDelta::days(365);

/// An app's request to connect to a user's account: pending until the user answers it,
/// then holding the hash of the authorization code until that expires.
#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryOAuthAuthorization {
    pub id: OAuthAuthorizationId,
    pub uuid: OAuthAuthorizationUuid,
    pub client_id: OAuthClientId,
    pub client_name: NonEmpty,
    /// Where the client asked to receive the answer, which it registered.
    pub redirect_uri: OAuthRedirectUri,
    pub scope: OAuthScopes,
    pub resource: OAuthResource,
    pub code_challenge: OAuthCodeChallenge,
    pub state: Option<OAuthRequestState>,
    pub nonce: Option<OAuthNonce>,
    /// Whether the client asked for refresh tokens.
    pub refresh: bool,
    pub user_id: Option<UserId>,
    /// The user key the request was approved with: a grant made from it dies with the key.
    pub user_key_id: Option<UserKeyId>,
    pub code_hash: Option<OAuthCodeHash>,
    pub grant_id: Option<OAuthGrantId>,
    pub created: DateTime,
    pub expiration: DateTime,
}

impl QueryOAuthAuthorization {
    pub fn from_uuid(
        conn: &mut DbConnection,
        uuid: OAuthAuthorizationUuid,
    ) -> diesel::QueryResult<Self> {
        schema::oauth_authorization::table
            .filter(schema::oauth_authorization::uuid.eq(uuid))
            .first(conn)
    }

    pub fn is_expired(&self, now: DateTime) -> bool {
        self.expiration.timestamp() <= now.timestamp()
    }

    /// Whether the user answered the request, which then holds an authorization code.
    pub fn is_answered(&self) -> bool {
        self.code_hash.is_some()
    }

    /// Whether the request is still waiting for the user's answer.
    pub fn is_pending(
        conn: &mut DbConnection,
        uuid: OAuthAuthorizationUuid,
        now: DateTime,
    ) -> diesel::QueryResult<bool> {
        Ok(Self::live(conn, uuid, now)?.is_some_and(|authorization| !authorization.is_answered()))
    }

    /// When the user approved an answered request: its code expires a fixed time after.
    pub fn approval_time(&self) -> DateTime {
        self.expiration - CODE
    }

    /// Approve a pending request for `user_id`, bound to `user_key_id` when the user approved with
    /// a key, recording the hash of the authorization code the app may exchange until it expires.
    pub fn approve(
        conn: &mut DbConnection,
        uuid: OAuthAuthorizationUuid,
        user_id: UserId,
        user_key_id: Option<UserKeyId>,
        code_hash: &OAuthCodeHash,
        now: DateTime,
    ) -> diesel::QueryResult<OAuthAnswer> {
        let Some(mut authorization) = Self::live(conn, uuid, now)? else {
            return Ok(OAuthAnswer::Missing);
        };
        if let Some(user_key_id) = user_key_id {
            let owned = schema::user_key::table
                .filter(schema::user_key::id.eq(user_key_id))
                .filter(schema::user_key::user_id.eq(user_id))
                .count()
                .get_result::<i64>(conn)?;
            if owned != 1 {
                return Ok(OAuthAnswer::ForeignKey);
            }
        }
        let expiration = now + CODE;
        let updated = diesel::update(
            schema::oauth_authorization::table
                .filter(schema::oauth_authorization::id.eq(authorization.id))
                .filter(schema::oauth_authorization::code_hash.is_null()),
        )
        .set((
            schema::oauth_authorization::user_id.eq(user_id),
            schema::oauth_authorization::user_key_id.eq(user_key_id),
            schema::oauth_authorization::code_hash.eq(code_hash),
            schema::oauth_authorization::expiration.eq(expiration),
        ))
        .execute(conn)?;
        if updated == 0 {
            return Ok(OAuthAnswer::Repeated);
        }
        authorization.user_id = Some(user_id);
        authorization.user_key_id = user_key_id;
        authorization.code_hash = Some(code_hash.clone());
        authorization.expiration = expiration;
        Ok(OAuthAnswer::Recorded(Box::new(authorization)))
    }

    /// Deny a pending request, which deletes it.
    pub fn deny(
        conn: &mut DbConnection,
        uuid: OAuthAuthorizationUuid,
        now: DateTime,
    ) -> diesel::QueryResult<OAuthAnswer> {
        let Some(authorization) = Self::live(conn, uuid, now)? else {
            return Ok(OAuthAnswer::Missing);
        };
        let deleted = diesel::delete(
            schema::oauth_authorization::table
                .filter(schema::oauth_authorization::id.eq(authorization.id))
                .filter(schema::oauth_authorization::code_hash.is_null()),
        )
        .execute(conn)?;
        Ok(if deleted == 0 {
            OAuthAnswer::Repeated
        } else {
            OAuthAnswer::Recorded(Box::new(authorization))
        })
    }

    /// An approved request by the hash of its authorization code, with the user who approved it
    /// and the user key they approved it with.
    pub fn approved(
        conn: &mut DbConnection,
        code_hash: &OAuthCodeHash,
    ) -> diesel::QueryResult<Option<(Self, QueryUser, Option<QueryUserKey>)>> {
        schema::oauth_authorization::table
            .inner_join(
                schema::user::table
                    .on(schema::oauth_authorization::user_id.eq(schema::user::id.nullable())),
            )
            .left_join(
                schema::user_key::table
                    .on(schema::oauth_authorization::user_key_id
                        .eq(schema::user_key::id.nullable())),
            )
            .filter(schema::oauth_authorization::code_hash.eq(code_hash))
            .select((
                schema::oauth_authorization::all_columns,
                schema::user::all_columns,
                schema::user_key::all_columns.nullable(),
            ))
            .first(conn)
            .optional()
    }

    /// Grant what the approved request asked for, recording that its code was exchanged.
    /// The grant lasts a year at most, and no longer than the user key it was approved with.
    pub fn grant(
        &self,
        conn: &mut DbConnection,
        query_user: &QueryUser,
        query_key: Option<&QueryUserKey>,
        now: DateTime,
    ) -> diesel::QueryResult<QueryOAuthGrant> {
        let expiration = query_key.map_or(now + GRANT, |query_key| {
            earliest(now + GRANT, query_key.expiration)
        });
        let uuid = OAuthGrantUuid::new();
        diesel::insert_into(schema::oauth_grant::table)
            .values((
                schema::oauth_grant::uuid.eq(uuid),
                schema::oauth_grant::user_id.eq(query_user.id),
                schema::oauth_grant::user_key_id.eq(self.user_key_id),
                schema::oauth_grant::client_id.eq(&self.client_id),
                schema::oauth_grant::client_name.eq(&self.client_name),
                schema::oauth_grant::scope.eq(&self.scope),
                schema::oauth_grant::resource.eq(&self.resource),
                schema::oauth_grant::created.eq(now),
                schema::oauth_grant::modified.eq(now),
                schema::oauth_grant::expiration.eq(expiration),
            ))
            .execute(conn)?;
        let query_grant = schema::oauth_grant::table
            .filter(schema::oauth_grant::uuid.eq(uuid))
            .first::<QueryOAuthGrant>(conn)?;
        diesel::update(
            schema::oauth_authorization::table.filter(schema::oauth_authorization::id.eq(self.id)),
        )
        .set(schema::oauth_authorization::grant_id.eq(query_grant.id))
        .execute(conn)?;
        Ok(query_grant)
    }

    pub fn into_json(self, oauth: &OAuthServer) -> JsonOAuthAuthorization {
        let Self {
            id: _,
            uuid,
            client_id,
            client_name,
            redirect_uri,
            scope,
            resource: _,
            code_challenge: _,
            state: _,
            nonce: _,
            refresh: _,
            user_id: _,
            user_key_id: _,
            code_hash: _,
            grant_id: _,
            created: _,
            expiration,
        } = self;
        JsonOAuthAuthorization {
            uuid,
            client: JsonOAuthAuthorizationClient {
                host: client_id.host(),
                verified: oauth.clients().is_verified(&client_id, &redirect_uri),
                id: client_id,
                name: client_name,
            },
            redirect: JsonOAuthAuthorizationRedirect {
                host: redirect_uri.host(),
                loopback: redirect_uri.is_loopback(),
            },
            scopes: scope.iter().collect(),
            expiration,
        }
    }

    /// The request, unless there is none or it expired.
    fn live(
        conn: &mut DbConnection,
        uuid: OAuthAuthorizationUuid,
        now: DateTime,
    ) -> diesel::QueryResult<Option<Self>> {
        Ok(Self::from_uuid(conn, uuid)
            .optional()?
            .filter(|authorization| !authorization.is_expired(now)))
    }
}

/// What became of a request once a user answered it.
#[derive(Debug)]
pub enum OAuthAnswer {
    /// The request was pending, and the answer is recorded: the request as it now stands.
    Recorded(Box<QueryOAuthAuthorization>),
    /// There is no such request, or it expired.
    Missing,
    /// The request was already answered.
    Repeated,
    /// The user key is not the approving user's.
    ForeignKey,
}

/// What an app asked for in its request to connect.
#[derive(Debug, Clone)]
pub struct NewOAuthAuthorization {
    pub client_id: OAuthClientId,
    pub client_name: NonEmpty,
    pub redirect_uri: OAuthRedirectUri,
    pub scope: OAuthScopes,
    pub resource: OAuthResource,
    pub code_challenge: OAuthCodeChallenge,
    pub state: Option<OAuthRequestState>,
    pub nonce: Option<OAuthNonce>,
    pub refresh: bool,
}

impl NewOAuthAuthorization {
    /// Record the request as pending from `now`, after dropping every expired request.
    /// Its UUID is random, since it is all a consent page needs to answer it.
    pub fn insert(
        self,
        conn: &mut DbConnection,
        now: DateTime,
    ) -> diesel::QueryResult<OAuthAuthorizationUuid> {
        diesel::delete(
            schema::oauth_authorization::table
                .filter(schema::oauth_authorization::expiration.le(now)),
        )
        .execute(conn)?;
        let Self {
            client_id,
            client_name,
            redirect_uri,
            scope,
            resource,
            code_challenge,
            state,
            nonce,
            refresh,
        } = self;
        let uuid = OAuthAuthorizationUuid::from(uuid::Uuid::new_v4());
        diesel::insert_into(schema::oauth_authorization::table)
            .values(&InsertOAuthAuthorization {
                uuid,
                client_id,
                client_name,
                redirect_uri,
                scope,
                resource,
                code_challenge,
                state,
                nonce,
                refresh,
                created: now,
                expiration: now + PENDING,
            })
            .execute(conn)?;
        Ok(uuid)
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = schema::oauth_authorization)]
struct InsertOAuthAuthorization {
    uuid: OAuthAuthorizationUuid,
    client_id: OAuthClientId,
    client_name: NonEmpty,
    redirect_uri: OAuthRedirectUri,
    scope: OAuthScopes,
    resource: OAuthResource,
    code_challenge: OAuthCodeChallenge,
    state: Option<OAuthRequestState>,
    nonce: Option<OAuthNonce>,
    refresh: bool,
    created: DateTime,
    expiration: DateTime,
}
