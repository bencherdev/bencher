use bencher_json::{
    DateTime, JsonOAuthClient, JsonOAuthGrant, JsonOAuthGrantKey, NonEmpty, OAuthAccessToken,
    OAuthClientId, OAuthCode, OAuthDevice, OAuthGrantUuid, OAuthRefreshToken, OAuthResource,
    OAuthScopes, OAuthTokenHash,
};
use chrono::TimeDelta;
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{
    context::{ApiContext, DbConnection, OAuthServer},
    error::{
        INVALID_CREDENTIAL, invalid_credential, issue_error, resource_not_found_err,
        unauthorized_error,
    },
    schema,
};

use super::{
    QueryUser, UserId,
    key::{QueryUserKey, UserKeyId},
};

/// How long an access token lasts.
const ACCESS_LIFETIME: TimeDelta = TimeDelta::hours(1);
/// How long a refresh token lasts. Each refresh issues a new one.
const REFRESH_LIFETIME: TimeDelta = TimeDelta::days(30);

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
    /// The device the Bencher CLI signed in on, by its hostname.
    pub device: Option<OAuthDevice>,
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

    pub fn from_id(conn: &mut DbConnection, grant_id: OAuthGrantId) -> diesel::QueryResult<Self> {
        schema::oauth_grant::table
            .filter(schema::oauth_grant::id.eq(grant_id))
            .first(conn)
    }

    /// Whether the grant itself is neither revoked nor expired, whatever its user and key.
    pub fn is_live(&self, now: DateTime) -> bool {
        self.revoked.is_none() && self.expiration.timestamp() > now.timestamp()
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

    /// Issue an access token, and a refresh token if asked, neither outliving the grant.
    pub fn issue(
        &self,
        conn: &mut DbConnection,
        refresh: RefreshTokens,
        now: DateTime,
    ) -> diesel::QueryResult<OAuthTokens> {
        let access_token = OAuthAccessToken::generate();
        let access_expiration = self.expiring(now + ACCESS_LIFETIME);
        self.insert_token(
            conn,
            OAuthTokenKind::Access,
            &OAuthTokenHash::from(&access_token),
            access_expiration,
            now,
        )?;
        let refresh_token = if refresh == RefreshTokens::Issue {
            let refresh_token = OAuthRefreshToken::generate();
            self.insert_token(
                conn,
                OAuthTokenKind::Refresh,
                &OAuthTokenHash::from(&refresh_token),
                self.expiring(now + REFRESH_LIFETIME),
                now,
            )?;
            Some(refresh_token)
        } else {
            None
        };
        Ok(OAuthTokens {
            access_token,
            access_expiration,
            refresh_token,
            scope: self.scope.clone(),
        })
    }

    /// Record that the grant was refreshed at `now`, dropping its expired tokens.
    pub fn refreshed(&self, conn: &mut DbConnection, now: DateTime) -> diesel::QueryResult<()> {
        diesel::update(schema::oauth_grant::table.filter(schema::oauth_grant::id.eq(self.id)))
            .set(schema::oauth_grant::modified.eq(now))
            .execute(conn)?;
        diesel::delete(
            schema::oauth_token::table
                .filter(schema::oauth_token::grant_id.eq(self.id))
                .filter(schema::oauth_token::expiration.le(now)),
        )
        .execute(conn)?;
        Ok(())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonOAuthGrant, HttpError> {
        let Self {
            id: _,
            uuid,
            user_id: _,
            user_key_id,
            client_id,
            client_name,
            device,
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
            device,
            scopes: scope.iter().collect(),
            user_key,
            created,
            modified,
            expiration,
        })
    }

    /// `expiration`, or the grant's own expiration if that comes first.
    fn expiring(&self, expiration: DateTime) -> DateTime {
        earliest(expiration, self.expiration)
    }

    fn insert_token(
        &self,
        conn: &mut DbConnection,
        kind: OAuthTokenKind,
        token_hash: &OAuthTokenHash,
        expiration: DateTime,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::insert_into(schema::oauth_token::table)
            .values((
                schema::oauth_token::grant_id.eq(self.id),
                schema::oauth_token::kind.eq(kind),
                schema::oauth_token::token_hash.eq(token_hash),
                schema::oauth_token::created.eq(now),
                schema::oauth_token::expiration.eq(expiration),
            ))
            .execute(conn)
    }
}

/// Whether a grant's client gets refresh tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTokens {
    Issue,
    Withhold,
}

/// The tokens issued for a grant, which are never stored, only their hashes.
pub struct OAuthTokens {
    pub access_token: OAuthAccessToken,
    pub access_expiration: DateTime,
    pub refresh_token: Option<OAuthRefreshToken>,
    pub scope: OAuthScopes,
}

impl QueryOAuthToken {
    /// A token by its hash, with what it acts for.
    pub fn with_grant(
        conn: &mut DbConnection,
        token_hash: &OAuthTokenHash,
    ) -> diesel::QueryResult<Option<OAuthTokenGrant>> {
        schema::oauth_token::table
            .inner_join(schema::oauth_grant::table.inner_join(schema::user::table))
            .left_join(
                schema::user_key::table.on(schema::oauth_grant::user_key_id
                    .eq(schema::user_key::id.nullable())
                    .and(schema::user_key::user_id.eq(schema::oauth_grant::user_id))),
            )
            .filter(schema::oauth_token::token_hash.eq(token_hash))
            .select((
                schema::oauth_token::all_columns,
                schema::oauth_grant::all_columns,
                schema::user::all_columns,
                schema::user_key::all_columns.nullable(),
            ))
            .first(conn)
            .optional()
    }

    /// Record that a refresh token was exchanged at `now`: presenting it again is reuse.
    pub fn use_up(&self, conn: &mut DbConnection, now: DateTime) -> diesel::QueryResult<usize> {
        diesel::update(schema::oauth_token::table.filter(schema::oauth_token::id.eq(self.id)))
            .set(schema::oauth_token::used.eq(now))
            .execute(conn)
    }

    pub fn delete(&self, conn: &mut DbConnection) -> diesel::QueryResult<usize> {
        diesel::delete(schema::oauth_token::table.filter(schema::oauth_token::id.eq(self.id)))
            .execute(conn)
    }

    pub fn is_expired(&self, now: DateTime) -> bool {
        self.expiration.timestamp() <= now.timestamp()
    }
}

/// A token with its grant, the grant's user, and the user key the grant is bound to
/// when that key is the user's.
pub type OAuthTokenGrant = (
    QueryOAuthToken,
    QueryOAuthGrant,
    QueryUser,
    Option<QueryUserKey>,
);

/// Why the Bencher CLI's access token is refused by the MCP server, which takes only outside apps'.
pub const CLI_TOKEN_AT_MCP: &str =
    "The Bencher CLI's OAuth tokens are accepted only by the REST API";
/// Why an outside app's access token is refused by the REST API, which takes only the Bencher
/// CLI's.
pub const APP_TOKEN_AT_REST: &str =
    "OAuth tokens of apps other than the Bencher CLI are accepted only by the MCP server";

/// Look up an OAuth access token presented to the MCP server or the user info endpoint.
///
/// A single query returns the token with its grant, user, and bound user key,
/// and every adverse case returns the same opaque error, but for a live token of the Bencher CLI,
/// which is told where it belongs.
pub fn authenticate(
    context: &ApiContext,
    conn: &mut DbConnection,
    oauth: &OAuthServer,
    token: &str,
) -> Result<(QueryUser, QueryOAuthGrant), HttpError> {
    let (query_token, query_grant, query_user, query_key) = lookup(conn, token)?;
    if let Some(failure) = token_failure(
        oauth,
        &query_token,
        &query_grant,
        &query_user,
        query_key.as_ref(),
        context.clock.now(),
    ) {
        return Err(if matches!(failure, OAuthTokenFailure::CliClient) {
            oauth_token_refusal(failure, CLI_TOKEN_AT_MCP)
        } else {
            oauth_token_failure(failure)
        });
    }
    Ok((query_user, query_grant))
}

/// Look up the Bencher CLI's OAuth access token presented to the REST API.
///
/// It needs no OAuth server configured. Every adverse case returns the same opaque error, but for
/// a live token of an outside app, which is told where it belongs.
pub fn authenticate_cli(
    context: &ApiContext,
    conn: &mut DbConnection,
    token: &OAuthAccessToken,
) -> Result<(QueryUser, QueryOAuthGrant), HttpError> {
    let (query_token, query_grant, query_user, query_key) = lookup(conn, token.as_ref())?;
    let now = context.clock.now();
    let failure = if query_token.kind != OAuthTokenKind::Access {
        Some(OAuthTokenFailure::Kind)
    } else if query_token.is_expired(now) {
        Some(OAuthTokenFailure::Expired)
    } else {
        grant_failure(&query_grant, &query_user, query_key.as_ref(), now).map(Into::into)
    };
    if let Some(failure) = failure {
        return Err(oauth_token_failure(failure));
    }
    if !query_grant.client_id.is_bencher_cli() {
        return Err(oauth_token_refusal(
            OAuthTokenFailure::RestApi,
            APP_TOKEN_AT_REST,
        ));
    }
    Ok((query_user, query_grant))
}

/// A token by its hash, with what it acts for, or the opaque error.
fn lookup(conn: &mut DbConnection, token: &str) -> Result<OAuthTokenGrant, HttpError> {
    let Some(token_hash) = oauth_token_hash(token) else {
        return Err(oauth_token_failure(OAuthTokenFailure::Invalid));
    };
    QueryOAuthToken::with_grant(conn, &token_hash)
        .inspect_err(|err| {
            issue_error(
                "Failed to lookup OAuth token",
                &format!("Failed to lookup OAuth token by hash: {token_hash}"),
                err,
            );
        })
        .map_err(|_err| invalid_credential())?
        .ok_or_else(|| oauth_token_failure(OAuthTokenFailure::NotFound))
}

/// Access and refresh tokens share a table, so either hashes to a lookup,
/// and the token's kind is then checked against what it was presented as.
pub fn oauth_token_hash(token: &str) -> Option<OAuthTokenHash> {
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

/// Whether a bearer credential is any OAuth token or authorization code. Of these, the REST API
/// takes only the Bencher CLI's access token.
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
    if query_token.kind != OAuthTokenKind::Access {
        return Some(OAuthTokenFailure::Kind);
    }
    if query_token.is_expired(now) {
        return Some(OAuthTokenFailure::Expired);
    }
    if let Some(failure) = grant_failure(query_grant, query_user, query_key, now) {
        return Some(failure.into());
    }
    if query_grant.client_id.is_bencher_cli() {
        return Some(OAuthTokenFailure::CliClient);
    }
    if !oauth.serves(&query_grant.resource) {
        return Some(OAuthTokenFailure::Resource);
    }
    if !oauth.clients().allows(&query_grant.client_id) {
        return Some(OAuthTokenFailure::UnlistedClient);
    }
    None
}

/// Why a grant can no longer act for its user, if it cannot.
pub fn grant_failure(
    query_grant: &QueryOAuthGrant,
    query_user: &QueryUser,
    query_key: Option<&QueryUserKey>,
    now: DateTime,
) -> Option<OAuthGrantFailure> {
    if query_grant.revoked.is_some() {
        return Some(OAuthGrantFailure::Revoked);
    }
    if query_grant.expiration.timestamp() <= now.timestamp() {
        return Some(OAuthGrantFailure::Expired);
    }
    approver_failure(query_user, query_grant.user_key_id, query_key, now)
}

/// Why a user, with the user key `user_key_id` they approved with if any, can no longer stand
/// behind an approval, if they cannot.
pub fn approver_failure(
    query_user: &QueryUser,
    user_key_id: Option<UserKeyId>,
    query_key: Option<&QueryUserKey>,
    now: DateTime,
) -> Option<OAuthGrantFailure> {
    if query_user.locked {
        return Some(OAuthGrantFailure::Locked);
    }
    if user_key_id.is_some() {
        // A bound key that did not join was deleted, which cascades, or is another user's.
        let Some(query_key) = query_key else {
            return Some(OAuthGrantFailure::KeyRevoked);
        };
        if query_key.revoked.is_some() {
            return Some(OAuthGrantFailure::KeyRevoked);
        }
        if query_key.expiration.timestamp() <= now.timestamp() {
            return Some(OAuthGrantFailure::KeyExpired);
        }
    }
    None
}

/// A live token refused where it does not belong, told why.
fn oauth_token_refusal(failure: OAuthTokenFailure, message: &str) -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenAuthFailed(
        failure.into(),
    ));
    let mut error = unauthorized_error(message);
    error.internal_message = format!("{message} ({failure:?})");
    error
}

fn oauth_token_failure(failure: OAuthTokenFailure) -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenAuthFailed(
        failure.into(),
    ));
    let mut error = invalid_credential();
    error.internal_message = format!("{INVALID_CREDENTIAL} ({failure:?})");
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
    UnlistedClient,
    Locked,
    KeyRevoked,
    KeyExpired,
    /// The Bencher CLI's live token, at the MCP server or the user info endpoint.
    CliClient,
    /// An outside app's live token, at the REST API.
    RestApi,
}

/// Why a grant can no longer act for its user.
#[derive(Debug, Clone, Copy)]
pub enum OAuthGrantFailure {
    Revoked,
    Expired,
    Locked,
    KeyRevoked,
    KeyExpired,
}

impl From<OAuthGrantFailure> for OAuthTokenFailure {
    fn from(failure: OAuthGrantFailure) -> Self {
        match failure {
            OAuthGrantFailure::Revoked => Self::GrantRevoked,
            OAuthGrantFailure::Expired => Self::GrantExpired,
            OAuthGrantFailure::Locked => Self::Locked,
            OAuthGrantFailure::KeyRevoked => Self::KeyRevoked,
            OAuthGrantFailure::KeyExpired => Self::KeyExpired,
        }
    }
}

/// Whichever of two times comes first.
pub fn earliest(left: DateTime, right: DateTime) -> DateTime {
    if right.timestamp() < left.timestamp() {
        right
    } else {
        left
    }
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
            OAuthTokenFailure::UnlistedClient => Self::UnlistedClient,
            OAuthTokenFailure::Locked => Self::Locked,
            OAuthTokenFailure::KeyRevoked => Self::KeyRevoked,
            OAuthTokenFailure::KeyExpired => Self::KeyExpired,
            OAuthTokenFailure::CliClient => Self::CliClient,
            OAuthTokenFailure::RestApi => Self::RestApi,
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
    use diesel_migrations::MigrationHarness as _;

    use crate::test_util::setup_test_db;

    const OAUTH_MIGRATION: &str = "20261012120000";
    const DEVICE_MIGRATION: &str = "20261127120000";

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

    // Kills a down migration that leaves the Bencher CLI's rows for a binary that cannot read its
    // client ID, or that drops an outside app's.
    #[test]
    fn device_migration_down_drops_only_the_cli() {
        let mut conn = setup_test_db();
        conn.batch_execute(
            "INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                VALUES (1, '00000000-0000-0000-0000-000000000080', 'User', 'user', 'user@example.com', 0, 0, 0, 0);
            INSERT INTO oauth_grant (id, uuid, user_id, client_id, client_name, device, scope, resource, created, modified, expiration)
                VALUES (1, '00000000-0000-0000-0000-000000000082', 1, 'https://client.example/client.json', 'Client', NULL, 'projects:read', 'https://api.example.com/mcp', 0, 0, 1),
                    (2, '00000000-0000-0000-0000-000000000083', 1, 'bencher_cli', 'Bencher CLI', 'ada-macbook', 'user', 'http://bencher.lan:6610', 0, 0, 1);
            INSERT INTO oauth_token (id, grant_id, kind, token_hash, created, expiration)
                VALUES (1, 1, 0, 'app', 0, 1), (2, 2, 0, 'cli', 0, 1), (3, 2, 1, 'cli refresh', 0, 1);
            INSERT INTO oauth_authorization (id, uuid, client_id, client_name, device, redirect_uri, scope, resource, code_challenge, refresh, grant_id, created, expiration)
                VALUES (1, '00000000-0000-0000-0000-000000000084', 'https://client.example/client.json', 'Client', NULL, 'https://client.example/callback', 'projects:read', 'https://api.example.com/mcp', 'challenge', 0, 1, 0, 1),
                    (2, '00000000-0000-0000-0000-000000000085', 'bencher_cli', 'Bencher CLI', 'ada-macbook', 'http://127.0.0.1:53682/callback', 'user', 'http://bencher.lan:6610', 'challenge', 1, 2, 0, 1),
                    (3, '00000000-0000-0000-0000-000000000086', 'bencher_cli', 'Bencher CLI', 'ada-macbook', 'http://127.0.0.1:53682/callback', 'user', 'http://bencher.lan:6610', 'challenge', 1, NULL, 0, 1);",
        )
        .unwrap();

        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == DEVICE_MIGRATION {
                break;
            }
        }
        for (table, left) in [
            ("oauth_grant", 1),
            ("oauth_token", 1),
            ("oauth_authorization", 1),
        ] {
            assert_eq!(
                count(&mut conn, &format!("SELECT COUNT(*) AS count FROM {table}")),
                left,
                "{table}"
            );
            assert_eq!(
                count(
                    &mut conn,
                    &format!("SELECT COUNT(*) AS count FROM {table} WHERE id = 1")
                ),
                1,
                "{table} keeps the outside app's row"
            );
        }

        conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        assert_eq!(
            count(
                &mut conn,
                "SELECT COUNT(*) AS count FROM oauth_grant WHERE device IS NULL"
            ),
            1
        );
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
