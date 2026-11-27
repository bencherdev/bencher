use bencher_json::{
    DateTime, JsonToken, JwtHash, ResourceName, TokenUuid, user::token::JsonUpdateToken,
};
use chrono::TimeDelta;
use diesel::{BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{BencherResource, assert_parentage, resource_not_found_err},
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    schema,
    schema::token as token_table,
};

use super::{QueryUser, UserId};

crate::macros::typed_id::typed_id!(TokenId);

/// How long a use of a token goes unrecorded after the last one was.
const USE_INTERVAL: TimeDelta = TimeDelta::hours(1);

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryToken {
    pub id: TokenId,
    pub uuid: TokenUuid,
    pub user_id: UserId,
    pub name: ResourceName,
    pub jwt_hash: JwtHash,
    pub creation: DateTime,
    pub expiration: DateTime,
    pub last_used: Option<DateTime>,
    pub revoked: Option<DateTime>,
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
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Token, (user_id, uuid)))
    }

    /// The token with this hash and the user it belongs to, whether or not it is still live.
    pub fn from_hash(
        conn: &mut DbConnection,
        jwt_hash: &JwtHash,
    ) -> diesel::QueryResult<(QueryUser, Self)> {
        schema::token::table
            .inner_join(schema::user::table)
            .filter(schema::token::jwt_hash.eq(jwt_hash))
            .select((schema::user::all_columns, schema::token::all_columns))
            .first::<(QueryUser, Self)>(conn)
    }

    pub fn is_expired(&self, now: DateTime) -> bool {
        self.expiration.timestamp() <= now.timestamp()
    }

    pub fn needs_use_recorded(&self, now: DateTime) -> bool {
        self.last_used
            .is_none_or(|last_used| now.timestamp() >= (last_used + USE_INTERVAL).timestamp())
    }

    /// Record a use of the token at `now`, unless one was recorded within the interval.
    pub fn record_use(
        conn: &mut DbConnection,
        id: TokenId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::token::table
                .filter(schema::token::id.eq(id))
                .filter(
                    schema::token::last_used
                        .is_null()
                        .or(schema::token::last_used.le(now - USE_INTERVAL)),
                ),
        )
        .set(schema::token::last_used.eq(now))
        .execute(conn)
    }

    pub fn revoke(
        conn: &mut DbConnection,
        token_id: TokenId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::token::table
                .filter(schema::token::id.eq(token_id))
                .filter(schema::token::revoked.is_null()),
        )
        .set(schema::token::revoked.eq(now))
        .execute(conn)
    }

    pub fn revoke_all(
        conn: &mut DbConnection,
        user_id: UserId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::token::table
                .filter(schema::token::user_id.eq(user_id))
                .filter(schema::token::revoked.is_null()),
        )
        .set(schema::token::revoked.eq(now))
        .execute(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonToken, HttpError> {
        let query_user = QueryUser::get(conn, self.user_id)?;
        Ok(self.into_json_for_user(&query_user))
    }

    pub fn into_json_for_user(self, query_user: &QueryUser) -> JsonToken {
        let Self {
            id: _,
            uuid,
            user_id,
            name,
            jwt_hash: _,
            creation,
            expiration,
            last_used: _,
            revoked,
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
            creation,
            expiration,
            revoked,
        }
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

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Email, Jwt, JwtHash, TokenUuid};
    use bencher_token::{DEFAULT_SECRET_KEY, TokenKey};
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        connection::SimpleConnection as _,
    };
    use diesel_migrations::MigrationHarness as _;
    use pretty_assertions::assert_eq;

    use super::{QueryToken, TokenId};
    use crate::{
        model::user::UserId, run_migrations, run_migrations_with_token_key, schema,
        test_util::setup_test_db,
    };

    const TOKEN_JWT_HASH_MIGRATION: &str = "20261110120000";
    const TOKENS: i64 = 64;

    #[derive(diesel::QueryableByName)]
    struct Text {
        #[diesel(sql_type = diesel::sql_types::Text)]
        text: String,
    }

    // Kills a migration that drops a row or a column's value, keeps the plaintext, stores
    // anything but each token's hash, revokes a live token, or hashes or judges again the hash
    // its down left behind.
    #[test]
    fn migration_hashes_every_token_in_place() {
        let mut conn = setup_test_db();
        revert_to_before_the_migration(&mut conn);
        let jwts = seed_tokens(&mut conn);
        let rows = "SELECT group_concat(concat_ws('|', id, uuid, user_id, name, creation, \
            expiration, revoked), ';') AS text FROM token";
        let before = text(&mut conn, rows);

        // The path the server takes at startup.
        run_migrations_with_token_key(&mut conn, Some(&token_key())).unwrap();
        let columns = "SELECT group_concat(name, ', ') AS text FROM pragma_table_info('token')";
        assert_eq!(
            text(&mut conn, columns),
            "id, uuid, user_id, name, jwt_hash, creation, expiration, last_used, revoked"
        );
        assert_eq!(text(&mut conn, rows), before);
        assert_hashed(&mut conn, &jwts);
        let unique = "SELECT group_concat(name) AS text FROM pragma_index_list('token') \
            WHERE \"unique\" = 1 AND name = 'index_token_jwt_hash'";
        assert_eq!(text(&mut conn, unique), "index_token_jwt_hash");

        revert_to_before_the_migration(&mut conn);
        assert_eq!(
            text(&mut conn, columns),
            "id, uuid, user_id, name, jwt, creation, expiration, revoked"
        );
        run_migrations_with_token_key(&mut conn, Some(&token_key())).unwrap();
        assert_eq!(text(&mut conn, rows), before);
        assert_hashed(&mut conn, &jwts);
    }

    // Kills a migration run with no server key that leaves a live token alive, which no key
    // vouches for, or that moves the time of a revocation already made.
    #[test]
    fn with_no_key_every_live_token_is_revoked() {
        let mut conn = setup_test_db();
        revert_to_before_the_migration(&mut conn);
        seed_tokens(&mut conn);

        let start = DateTime::now().timestamp();
        run_migrations(&mut conn).unwrap();
        let migrated = start..=DateTime::now().timestamp();
        let rows: Vec<(i32, Option<i64>)> = schema::token::table
            .select((schema::token::id, schema::token::revoked))
            .order(schema::token::id)
            .load(&mut conn)
            .unwrap();
        assert_eq!(rows.len(), usize::try_from(TOKENS).unwrap());
        for (id, revoked) in rows {
            let revoked = revoked.unwrap();
            if id % 2 == 0 {
                assert!(migrated.contains(&revoked), "token {id}");
            } else {
                assert_eq!(revoked, 1_700_000_100 + i64::from(id), "token {id}");
            }
        }
    }

    // Kills a migration that frees the plaintext table's pages with its tokens still in them.
    #[test]
    fn migration_leaves_no_token_in_the_file() {
        assert_no_token_in_the_file(0);
    }

    // Kills a merge that deletes the copies of a token with `secure_delete` off, which frees
    // their pages to the hashed table with the token still in them.
    #[test]
    fn merged_copies_leave_no_token_in_the_file() {
        assert_no_token_in_the_file(2);
    }

    // Kills a migration that fails on copies of a token, or keeps any copy but the one with the
    // lowest id, or leaves the hash without its unique index.
    #[test]
    fn copies_of_a_token_merge_into_the_lowest_id() {
        let mut conn = setup_test_db();
        revert_to_before_the_migration(&mut conn);
        let jwts = seed_tokens(&mut conn);
        copy_tokens(&mut conn, 2);

        run_migrations(&mut conn).unwrap();
        assert_eq!(
            text(
                &mut conn,
                "SELECT group_concat(id) AS text FROM (SELECT id FROM token ORDER BY id)"
            ),
            (1..=TOKENS)
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        for (id, jwt) in &jwts {
            let (_, query_token) = QueryToken::from_hash(&mut conn, &JwtHash::from(jwt)).unwrap();
            assert_eq!(i32::from(query_token.id), *id);
        }
        let unique = "SELECT group_concat(name) AS text FROM pragma_index_list('token') \
            WHERE \"unique\" = 1 AND name = 'index_token_jwt_hash'";
        assert_eq!(text(&mut conn, unique), "index_token_jwt_hash");
    }

    // Kills a merge that keeps the lowest id's own revocation, or any time but the earliest of
    // its copies'.
    #[test]
    fn a_merged_token_is_revoked_at_its_earliest_revocation() {
        let mut conn = setup_test_db();
        revert_to_before_the_migration(&mut conn);
        seed_tokens(&mut conn);
        copy_tokens(&mut conn, 2);
        // The second copy of each token was revoked before the token and its first copy were.
        conn.batch_execute(&format!(
            "UPDATE token SET revoked = 1600000000 + id WHERE id > {}",
            2 * TOKENS
        ))
        .unwrap();

        run_migrations(&mut conn).unwrap();
        assert_eq!(
            text(
                &mut conn,
                "SELECT group_concat(revoked) AS text FROM (SELECT revoked FROM token ORDER BY id)"
            ),
            (1..=TOKENS)
                .map(|id| (1_600_000_000 + 2 * TOKENS + id).to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    // Kills a write that records a use within an hour of the last, which racing requests would
    // otherwise both make, or one that never records it again.
    #[test]
    fn record_use_writes_at_most_once_an_hour() {
        let mut conn = setup_test_db();
        conn.batch_execute(
            "INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                VALUES (1, '00000000-0000-4000-8000-000000000001', 'User', 'user',
                    'user@example.com', 0, 0, 1700000000, 1700000000);
            INSERT INTO token (id, uuid, user_id, name, jwt_hash, creation, expiration)
                VALUES (1, '00000000-0000-4000-a000-000000000001', 1, 'Token',
                    '0000000000000000000000000000000000000000000000000000000000000000',
                    1700000000, 1900000000);",
        )
        .unwrap();
        let id = TokenId::from_raw(1);
        let at = |minutes: i64| DateTime::try_from(1_700_000_000 + minutes * 60).unwrap();
        for (minutes, written) in [(0, 1), (0, 0), (59, 0), (60, 1), (119, 0), (120, 1)] {
            assert_eq!(
                QueryToken::record_use(&mut conn, id, at(minutes)).unwrap(),
                written,
                "{minutes} minutes in"
            );
        }
    }

    // Kills a check that asks for a write on every use, or not until after the hour is up.
    #[test]
    fn needs_use_recorded_once_the_hour_is_up() {
        let at = |minutes: i64| DateTime::try_from(1_700_000_000 + minutes * 60).unwrap();
        let mut query_token = QueryToken {
            id: TokenId::from_raw(1),
            uuid: TokenUuid::new(),
            user_id: UserId::from_raw(1),
            name: "Token".parse().unwrap(),
            jwt_hash: "0".repeat(64).parse().unwrap(),
            creation: at(0),
            expiration: at(1_000_000),
            last_used: None,
            revoked: None,
        };
        assert!(query_token.needs_use_recorded(at(0)));
        query_token.last_used = Some(at(0));
        for (minutes, needed) in [(0, false), (59, false), (60, true), (61, true)] {
            assert_eq!(
                query_token.needs_use_recorded(at(minutes)),
                needed,
                "{minutes} minutes in"
            );
        }
    }

    /// Every token has its JWT's hash, no column or value holds a JWT, and each is found by it.
    fn assert_hashed(conn: &mut diesel::SqliteConnection, jwts: &[(i32, Jwt)]) {
        let all = text(
            conn,
            "SELECT group_concat(concat_ws('|', id, uuid, user_id, name, jwt_hash, creation, \
                expiration, last_used, revoked), ';') AS text FROM token",
        );
        for (id, jwt) in jwts {
            let hash = JwtHash::from(jwt);
            let stored: JwtHash = schema::token::table
                .filter(schema::token::id.eq(id))
                .select(schema::token::jwt_hash)
                .first(conn)
                .unwrap();
            assert_eq!(stored, hash);
            let (query_user, query_token) = QueryToken::from_hash(conn, &hash).unwrap();
            assert_eq!(i32::from(query_token.id), *id);
            assert_eq!(query_user.id, query_token.user_id);
            assert_eq!(query_token.last_used, None);
            for part in jwt.as_ref().split('.') {
                assert!(!all.contains(part), "{all}");
            }
        }
        assert_eq!(jwts.len(), usize::try_from(TOKENS).unwrap());
    }

    /// No token's signature is in the database file once its tokens, each with `copies` copies,
    /// are migrated.
    fn assert_no_token_in_the_file(copies: i64) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bencher.db");
        let mut conn = diesel::SqliteConnection::establish(path.to_str().unwrap()).unwrap();
        run_migrations(&mut conn).unwrap();
        revert_to_before_the_migration(&mut conn);
        let jwts = seed_tokens(&mut conn);
        copy_tokens(&mut conn, copies);
        run_migrations(&mut conn).unwrap();
        drop(conn);

        let file = std::fs::read(&path).unwrap();
        for (id, jwt) in &jwts {
            let (_, signature) = jwt.as_ref().rsplit_once('.').unwrap();
            assert!(
                !file
                    .windows(signature.len())
                    .any(|window| window == signature.as_bytes()),
                "token {id}"
            );
        }
    }

    /// `copies` more rows of every seeded token, each its user's with its JWT, as two requests in
    /// one second made them, after all of the seeded ones.
    fn copy_tokens(conn: &mut diesel::SqliteConnection, copies: i64) {
        conn.batch_execute(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {copies})
            INSERT INTO token (id, uuid, user_id, name, jwt, creation, expiration, revoked)
                SELECT i * {TOKENS} + id, printf('00000000-0000-4000-b000-%012d', i * {TOKENS} + id),
                    user_id, name, jwt, creation, expiration, revoked
                FROM token, n
                WHERE i <= {copies};"
        ))
        .unwrap();
    }

    fn revert_to_before_the_migration(conn: &mut diesel::SqliteConnection) {
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == TOKEN_JWT_HASH_MIGRATION {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
    }

    /// Users, each with a deprecated API token in plaintext, every other one revoked.
    fn seed_tokens(conn: &mut diesel::SqliteConnection) -> Vec<(i32, Jwt)> {
        let token_key = token_key();
        conn.batch_execute(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {TOKENS})
            INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                SELECT i, printf('00000000-0000-4000-8000-%012d', i), 'User ' || i,
                    'user-' || i, 'user' || i || '@example.com', 0, 0, 1700000000, 1700000000
                FROM n;"
        ))
        .unwrap();
        (1..=TOKENS)
            .map(|i| {
                let id = i32::try_from(i).unwrap();
                let email: Email = format!("user{i}@example.com").parse().unwrap();
                let jwt = token_key.new_api_key(email, u32::MAX).unwrap();
                let revoked = if i % 2 == 0 {
                    "NULL".to_owned()
                } else {
                    (1_700_000_100 + i).to_string()
                };
                conn.batch_execute(&format!(
                    "INSERT INTO token (id, uuid, user_id, name, jwt, creation, expiration, revoked)
                        VALUES ({id}, printf('00000000-0000-4000-a000-%012d', {id}), {id},
                            'Token {id}', '{jwt}', 1700000000, {expiration}, {revoked})",
                    jwt = jwt.as_ref(),
                    expiration = 1_800_000_000 + i,
                ))
                .unwrap();
                (id, jwt)
            })
            .collect()
    }

    /// The key the seeded tokens are issued and signed by, as a server's would be.
    fn token_key() -> TokenKey {
        TokenKey::new("https://bencher.dev/".to_owned(), &DEFAULT_SECRET_KEY)
    }

    fn text(conn: &mut diesel::SqliteConnection, query: &str) -> String {
        diesel::sql_query(query)
            .get_result::<Text>(conn)
            .unwrap()
            .text
    }
}
