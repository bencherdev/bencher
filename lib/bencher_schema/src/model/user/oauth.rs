use bencher_json::{
    DateTime, JsonOAuthClient, JsonOAuthGrant, JsonOAuthGrantKey, NonEmpty, OAuthAccessToken,
    OAuthClientId, OAuthCode, OAuthGrantUuid, OAuthRefreshToken, OAuthResource, OAuthScopes,
    OAuthTokenHash,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{
    context::{ApiContext, DbConnection, OAuthServer},
    error::{issue_error, resource_not_found_err, unauthorized_error},
    schema,
};

use super::{
    QueryUser, UserId,
    key::{QueryUserKey, UserKeyId},
};

const INVALID_OAUTH_TOKEN: &str = "Invalid OAuth access token";

crate::macros::typed_id::typed_id!(OAuthGrantId);
crate::macros::typed_id::typed_id!(OAuthTokenId);

/// A user's consent for one client to act on their behalf, held until revoked or expired.
#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryOAuthGrant {
    pub id: OAuthGrantId,
    pub uuid: OAuthGrantUuid,
    pub user_id: UserId,
    /// The user key the grant was approved with: the grant dies with it.
    pub user_key_id: Option<UserKeyId>,
    pub client_id: OAuthClientId,
    pub client_name: NonEmpty,
    pub scope: OAuthScopes,
    pub resource: OAuthResource,
    pub created: DateTime,
    pub modified: DateTime,
    pub expiration: DateTime,
    pub revoked: Option<DateTime>,
}

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryOAuthToken {
    pub id: OAuthTokenId,
    pub grant_id: OAuthGrantId,
    pub kind: OAuthTokenKind,
    pub token_hash: OAuthTokenHash,
    pub created: DateTime,
    pub expiration: DateTime,
    pub used: Option<DateTime>,
}

impl QueryOAuthGrant {
    /// A user's grants that can still be used: not revoked, not expired,
    /// and approved with no user key or with one that is still live.
    pub fn active(
        user_id: UserId,
        now: DateTime,
    ) -> schema::oauth_grant::BoxedQuery<'static, diesel::sqlite::Sqlite> {
        let live_keys = schema::user_key::table
            .filter(schema::user_key::user_id.eq(user_id))
            .filter(schema::user_key::revoked.is_null())
            .filter(schema::user_key::expiration.gt(now))
            .select(schema::user_key::id.nullable());
        schema::oauth_grant::table
            .filter(schema::oauth_grant::user_id.eq(user_id))
            .filter(schema::oauth_grant::revoked.is_null())
            .filter(schema::oauth_grant::expiration.gt(now))
            .filter(
                schema::oauth_grant::user_key_id
                    .is_null()
                    .or(schema::oauth_grant::user_key_id.eq_any(live_keys)),
            )
            .into_boxed()
    }

    pub fn get_active(
        conn: &mut DbConnection,
        user_id: UserId,
        uuid: OAuthGrantUuid,
        now: DateTime,
    ) -> Result<Self, HttpError> {
        Self::active(user_id, now)
            .filter(schema::oauth_grant::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(OAuthGrant, (user_id, uuid)))
    }

    pub fn revoke(
        conn: &mut DbConnection,
        grant_id: OAuthGrantId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::oauth_grant::table
                .filter(schema::oauth_grant::id.eq(grant_id))
                .filter(schema::oauth_grant::revoked.is_null()),
        )
        .set(schema::oauth_grant::revoked.eq(now))
        .execute(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonOAuthGrant, HttpError> {
        let Self {
            id: _,
            uuid,
            user_id: _,
            user_key_id,
            client_id,
            client_name,
            scope,
            resource: _,
            created,
            modified,
            expiration,
            revoked: _,
        } = self;
        let user_key = user_key_id
            .map(|user_key_id| QueryUserKey::get(conn, user_key_id))
            .transpose()?
            .map(|query_key| JsonOAuthGrantKey {
                uuid: query_key.uuid,
                name: query_key.name,
            });
        Ok(JsonOAuthGrant {
            uuid,
            client: JsonOAuthClient {
                host: client_id.host(),
                id: client_id,
                name: client_name,
            },
            scopes: scope.iter().collect(),
            user_key,
            created,
            modified,
            expiration,
        })
    }
}

/// Look up an OAuth access token presented to the MCP server with its grant and user.
/// A single query returns the token with its grant, user, and bound user key,
/// and every adverse case returns the same opaque error.
pub(super) fn authenticate(
    context: &ApiContext,
    conn: &mut DbConnection,
    oauth: &OAuthServer,
    token: &str,
) -> Result<(QueryUser, QueryOAuthGrant), HttpError> {
    let Some(token_hash) = oauth_token_hash(token) else {
        return Err(oauth_token_failure(OAuthTokenFailure::Invalid));
    };
    let lookup = schema::oauth_token::table
        .inner_join(schema::oauth_grant::table.inner_join(schema::user::table))
        .left_join(
            schema::user_key::table.on(schema::oauth_grant::user_key_id
                .eq(schema::user_key::id.nullable())
                .and(schema::user_key::user_id.eq(schema::oauth_grant::user_id))),
        )
        .filter(schema::oauth_token::token_hash.eq(&token_hash))
        .select((
            schema::oauth_token::all_columns,
            schema::oauth_grant::all_columns,
            schema::user::all_columns,
            schema::user_key::all_columns.nullable(),
        ))
        .first::<(
            QueryOAuthToken,
            QueryOAuthGrant,
            QueryUser,
            Option<QueryUserKey>,
        )>(conn)
        .optional()
        .inspect_err(|err| {
            issue_error(
                "Failed to lookup OAuth token",
                &format!("Failed to lookup OAuth token by hash: {token_hash}"),
                err,
            );
        })
        .map_err(|_err| unauthorized_error(INVALID_OAUTH_TOKEN))?;
    let Some((query_token, query_grant, query_user, query_key)) = lookup else {
        return Err(oauth_token_failure(OAuthTokenFailure::NotFound));
    };

    if let Some(failure) = token_failure(
        oauth,
        &query_token,
        &query_grant,
        &query_user,
        query_key.as_ref(),
        context.clock.now(),
    ) {
        return Err(oauth_token_failure(failure));
    }
    Ok((query_user, query_grant))
}

/// Access and refresh tokens share a table, so either hashes to a lookup;
/// only an access token is then accepted.
fn oauth_token_hash(token: &str) -> Option<OAuthTokenHash> {
    if let Ok(access_token) = token.parse::<OAuthAccessToken>() {
        Some(OAuthTokenHash::from(&access_token))
    } else if let Ok(refresh_token) = token.parse::<OAuthRefreshToken>() {
        Some(OAuthTokenHash::from(&refresh_token))
    } else {
        None
    }
}

/// Whether a bearer credential is an OAuth token rather than a Bencher key or JWT.
pub fn is_oauth_token(token: &str) -> bool {
    token.starts_with(OAuthAccessToken::PREFIX) || token.starts_with(OAuthRefreshToken::PREFIX)
}

/// Whether a bearer credential is any OAuth token or authorization code, none of which a REST
/// endpoint accepts.
pub fn is_oauth_credential(raw: &str) -> bool {
    is_oauth_token(raw) || raw.starts_with(OAuthCode::PREFIX)
}

fn token_failure(
    oauth: &OAuthServer,
    query_token: &QueryOAuthToken,
    query_grant: &QueryOAuthGrant,
    query_user: &QueryUser,
    query_key: Option<&QueryUserKey>,
    now: DateTime,
) -> Option<OAuthTokenFailure> {
    let now = now.timestamp();
    if query_token.kind != OAuthTokenKind::Access {
        return Some(OAuthTokenFailure::Kind);
    }
    if query_token.expiration.timestamp() <= now {
        return Some(OAuthTokenFailure::Expired);
    }
    if query_grant.revoked.is_some() {
        return Some(OAuthTokenFailure::GrantRevoked);
    }
    if query_grant.expiration.timestamp() <= now {
        return Some(OAuthTokenFailure::GrantExpired);
    }
    if !oauth.serves(&query_grant.resource) {
        return Some(OAuthTokenFailure::Resource);
    }
    if query_user.locked {
        return Some(OAuthTokenFailure::Locked);
    }
    if query_grant.user_key_id.is_some() {
        // Only the grant's own user's key joins, and a deleted key cascades to its grants.
        let Some(query_key) = query_key else {
            return Some(OAuthTokenFailure::KeyRevoked);
        };
        if query_key.revoked.is_some() {
            return Some(OAuthTokenFailure::KeyRevoked);
        }
        if query_key.expiration.timestamp() <= now {
            return Some(OAuthTokenFailure::KeyExpired);
        }
    }
    None
}

fn oauth_token_failure(failure: OAuthTokenFailure) -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenAuthFailed(
        failure.into(),
    ));
    let mut error = unauthorized_error(INVALID_OAUTH_TOKEN);
    error.internal_message = format!("{INVALID_OAUTH_TOKEN} ({failure:?})");
    error
}

/// What an OAuth token row grants: an access token is a bearer credential for the MCP server,
/// a refresh token is exchanged for a new access token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub enum OAuthTokenKind {
    Access,
    Refresh,
}

const ACCESS_INT: i32 = 0;
const REFRESH_INT: i32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum OAuthTokenKindError {
    #[error("Invalid OAuth token kind value: {0}")]
    Invalid(i32),
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for OAuthTokenKind
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Access => ACCESS_INT.to_sql(out),
            Self::Refresh => REFRESH_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for OAuthTokenKind
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            ACCESS_INT => Ok(Self::Access),
            REFRESH_INT => Ok(Self::Refresh),
            value => Err(Box::new(OAuthTokenKindError::Invalid(value))),
        }
    }
}

/// Why an OAuth token was refused, for telemetry only: the client always sees the same error.
#[derive(Debug, Clone, Copy)]
enum OAuthTokenFailure {
    Invalid,
    NotFound,
    Kind,
    Expired,
    GrantRevoked,
    GrantExpired,
    Resource,
    Locked,
    KeyRevoked,
    KeyExpired,
}

#[cfg(feature = "otel")]
impl From<OAuthTokenFailure> for bencher_otel::OAuthTokenAuthFailureReason {
    fn from(failure: OAuthTokenFailure) -> Self {
        match failure {
            OAuthTokenFailure::Invalid => Self::Invalid,
            OAuthTokenFailure::NotFound => Self::NotFound,
            OAuthTokenFailure::Kind => Self::Kind,
            OAuthTokenFailure::Expired => Self::Expired,
            OAuthTokenFailure::GrantRevoked => Self::GrantRevoked,
            OAuthTokenFailure::GrantExpired => Self::GrantExpired,
            OAuthTokenFailure::Resource => Self::Resource,
            OAuthTokenFailure::Locked => Self::Locked,
            OAuthTokenFailure::KeyRevoked => Self::KeyRevoked,
            OAuthTokenFailure::KeyExpired => Self::KeyExpired,
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
    use diesel_migrations::MigrationHarness as _;

    use crate::test_util::setup_test_db;

    const OAUTH_MIGRATION: &str = "20261012120000";

    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    fn count(conn: &mut diesel::SqliteConnection, query: &str) -> i64 {
        diesel::sql_query(query)
            .get_result::<Count>(conn)
            .unwrap()
            .count
    }

    #[test]
    fn migration_down_and_up_keeps_users_and_keys() {
        let mut conn = setup_test_db();
        conn.batch_execute(
            "INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                VALUES (1, '00000000-0000-0000-0000-000000000080', 'User', 'user', 'user@example.com', 0, 0, 0, 0);
            INSERT INTO user_key (id, uuid, user_id, name, key_hash, creation, expiration)
                VALUES (1, '00000000-0000-0000-0000-000000000081', 1, 'Key', 'hash', 0, 1);
            INSERT INTO oauth_grant (id, uuid, user_id, user_key_id, client_id, client_name, scope, resource, created, modified, expiration)
                VALUES (1, '00000000-0000-0000-0000-000000000082', 1, 1, 'https://client.example/client.json', 'Client', 'projects:read', 'https://api.example.com/mcp', 0, 0, 1);
            INSERT INTO oauth_token (id, grant_id, kind, token_hash, created, expiration)
                VALUES (1, 1, 0, 'hash', 0, 1);",
        )
        .unwrap();

        // Foreign keys are off as in `run_migrations`, so a later migration that rebuilds a table reverts.
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == OAUTH_MIGRATION {
                break;
            }
        }
        assert_eq!(
            count(
                &mut conn,
                "SELECT COUNT(*) AS count FROM sqlite_master WHERE name LIKE '%oauth%'"
            ),
            0,
            "the down migration drops every OAuth table and index"
        );
        assert_eq!(count(&mut conn, "SELECT COUNT(*) AS count FROM user"), 1);
        assert_eq!(
            count(&mut conn, "SELECT COUNT(*) AS count FROM user_key"),
            1
        );

        conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        assert_eq!(count(&mut conn, "SELECT COUNT(*) AS count FROM user"), 1);
        assert_eq!(
            count(&mut conn, "SELECT COUNT(*) AS count FROM user_key"),
            1
        );
        assert_eq!(
            count(&mut conn, "SELECT COUNT(*) AS count FROM oauth_grant"),
            0,
            "the up migration restores empty tables"
        );
    }
}
