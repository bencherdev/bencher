use bencher_json::{
    DateTime, JsonSession, JsonUser, NonEmpty, SessionMethod, SessionToken, SessionTokenHash,
    SessionUuid,
};
use chrono::TimeDelta;
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::issue_error,
    schema::{self, session as session_table},
};

use super::{QueryUser, UserId};

pub mod cookie;

crate::macros::typed_id::typed_id!(SessionId);

/// A session ends this long after its last use.
pub const SESSION_IDLE_LIFETIME: TimeDelta = TimeDelta::days(30);
/// A session ends this long after it began, however often it is used.
pub const SESSION_LIFETIME: TimeDelta = TimeDelta::days(90);
/// A use renews a session only once this long has passed since the last renewal.
const RENEWAL_INTERVAL: TimeDelta = TimeDelta::hours(1);

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QuerySession {
    pub id: SessionId,
    pub uuid: SessionUuid,
    pub user_id: UserId,
    pub token_hash: SessionTokenHash,
    pub method: SessionMethod,
    /// Where the reader signed in, such as "Firefox on macOS".
    pub device: Option<NonEmpty>,
    pub created: DateTime,
    /// When the reader last proved who they are.
    pub proved: DateTime,
    pub last_used: DateTime,
}

impl QuerySession {
    /// The session, unless it has been deleted.
    pub fn find(conn: &mut DbConnection, id: SessionId) -> diesel::QueryResult<Option<Self>> {
        schema::session::table
            .filter(schema::session::id.eq(id))
            .first(conn)
            .optional()
    }

    /// The session with the token's hash and its user, whether or not it has ended.
    pub fn from_token(
        conn: &mut DbConnection,
        token: &SessionToken,
    ) -> diesel::QueryResult<(QueryUser, Self)> {
        schema::session::table
            .inner_join(schema::user::table)
            .filter(schema::session::token_hash.eq(SessionTokenHash::from(token)))
            .select((schema::user::all_columns, schema::session::all_columns))
            .first::<(QueryUser, Self)>(conn)
    }

    /// The earlier of its idle end and its absolute end.
    pub fn expiration(&self) -> DateTime {
        let idle = self.last_used + SESSION_IDLE_LIFETIME;
        let absolute = self.created + SESSION_LIFETIME;
        if idle.timestamp() <= absolute.timestamp() {
            idle
        } else {
            absolute
        }
    }

    pub fn is_expired(&self, now: DateTime) -> bool {
        now.timestamp() >= self.expiration().timestamp()
    }

    pub fn needs_renewal(&self, now: DateTime) -> bool {
        now.timestamp() >= (self.last_used + RENEWAL_INTERVAL).timestamp()
    }

    pub fn renew(
        conn: &mut DbConnection,
        id: SessionId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(schema::session::table.filter(schema::session::id.eq(id)))
            .set(schema::session::last_used.eq(now))
            .execute(conn)
    }

    pub fn delete(conn: &mut DbConnection, id: SessionId) -> diesel::QueryResult<usize> {
        diesel::delete(schema::session::table.filter(schema::session::id.eq(id))).execute(conn)
    }

    pub fn delete_all(conn: &mut DbConnection, user_id: UserId) -> diesel::QueryResult<usize> {
        diesel::delete(schema::session::table.filter(schema::session::user_id.eq(user_id)))
            .execute(conn)
    }

    pub fn into_json(self, user: JsonUser) -> JsonSession {
        let expiration = self.expiration();
        let Self {
            id: _,
            uuid: _,
            user_id: _,
            token_hash: _,
            method,
            device: _,
            created,
            proved: _,
            last_used: _,
        } = self;
        JsonSession {
            user,
            method,
            created,
            expiration,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = session_table)]
pub struct InsertSession {
    pub uuid: SessionUuid,
    pub user_id: UserId,
    pub token_hash: SessionTokenHash,
    pub method: SessionMethod,
    pub device: Option<NonEmpty>,
    pub created: DateTime,
    pub proved: DateTime,
    pub last_used: DateTime,
}

impl InsertSession {
    /// A session for a reader who proved who they are at `now`, and its token, which no row holds.
    pub fn new(
        user_id: UserId,
        method: SessionMethod,
        user_agent: Option<&str>,
        now: DateTime,
    ) -> Result<(Self, SessionToken), HttpError> {
        let token = SessionToken::generate().map_err(|e| {
            issue_error(
                "Failed to generate a session token",
                "Failed to generate a session token:",
                e,
            )
        })?;
        let session = Self {
            uuid: SessionUuid::new(),
            user_id,
            token_hash: SessionTokenHash::from(&token),
            method,
            device: user_agent.and_then(device_label),
            created: now,
            proved: now,
            last_used: now,
        };
        Ok((session, token))
    }

    /// Record the session, after dropping every session that has ended.
    pub fn insert(&self, conn: &mut DbConnection, now: DateTime) -> diesel::QueryResult<()> {
        diesel::delete(
            schema::session::table.filter(
                schema::session::last_used
                    .le(now - SESSION_IDLE_LIFETIME)
                    .or(schema::session::created.le(now - SESSION_LIFETIME)),
            ),
        )
        .execute(conn)?;
        diesel::insert_into(schema::session::table)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

/// A label for the device a `User-Agent` names, such as "Firefox on macOS",
/// or none when it names neither a known browser nor a known operating system.
fn device_label(user_agent: &str) -> Option<NonEmpty> {
    let has = |token: &str| user_agent.contains(token);
    let browser = if has("Edg/") || has("EdgA/") || has("EdgiOS/") {
        Some("Edge")
    } else if has("OPR/") || has("Opera") {
        Some("Opera")
    } else if has("Firefox/") || has("FxiOS/") {
        Some("Firefox")
    } else if has("Chrome/") || has("CriOS/") || has("Chromium/") {
        Some("Chrome")
    } else if has("Safari/") {
        Some("Safari")
    } else {
        None
    };
    let os = if has("iPhone") || has("iPad") || has("iPod") {
        Some("iOS")
    } else if has("Android") {
        Some("Android")
    } else if has("CrOS") {
        Some("ChromeOS")
    } else if has("Windows") {
        Some("Windows")
    } else if has("Macintosh") || has("Mac OS X") {
        Some("macOS")
    } else if has("Linux") {
        Some("Linux")
    } else {
        None
    };
    let label = match (browser, os) {
        (Some(browser), Some(os)) => format!("{browser} on {os}"),
        (Some(name), None) | (None, Some(name)) => name.to_owned(),
        (None, None) => return None,
    };
    label.parse().ok()
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, SessionMethod};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use pretty_assertions::assert_eq;

    use super::{InsertSession, SESSION_IDLE_LIFETIME, SESSION_LIFETIME, device_label};
    use crate::{model::user::UserId, schema, test_util::setup_test_db};

    #[test]
    fn device_label_names_browser_and_os() {
        for (user_agent, label) in [
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.6; rv:131.0) Gecko/20100101 Firefox/131.0",
                Some("Firefox on macOS"),
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
                Some("Chrome on Windows"),
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
                Some("Edge on Windows"),
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
                Some("Safari on iOS"),
            ),
            (
                "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36",
                Some("Chrome on Android"),
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0",
                Some("Firefox on Linux"),
            ),
            ("curl/8.7.1", None),
        ] {
            assert_eq!(
                device_label(user_agent).as_ref().map(AsRef::as_ref),
                label,
                "{user_agent}"
            );
        }
    }

    // A new session sweeps out every session that has ended, idle or absolute, and no other.
    #[test]
    fn insert_drops_ended_sessions() {
        let mut conn = setup_test_db();
        let user_id = seed_user(&mut conn);
        let now = DateTime::TEST;
        let idle = session(
            user_id,
            now - SESSION_IDLE_LIFETIME,
            now - SESSION_IDLE_LIFETIME,
        );
        let absolute = session(user_id, now - SESSION_LIFETIME, now);
        let live = session(
            user_id,
            now - SESSION_LIFETIME + chrono::TimeDelta::seconds(1),
            now - SESSION_IDLE_LIFETIME + chrono::TimeDelta::seconds(1),
        );
        for old in [&idle, &absolute, &live] {
            diesel::insert_into(schema::session::table)
                .values(old)
                .execute(&mut conn)
                .unwrap();
        }

        let (new, _token) = InsertSession::new(user_id, SessionMethod::Email, None, now).unwrap();
        new.insert(&mut conn, now).unwrap();

        let left = schema::session::table
            .select(schema::session::uuid)
            .order(schema::session::id)
            .load::<bencher_json::SessionUuid>(&mut conn)
            .unwrap();
        assert_eq!(left, vec![live.uuid, new.uuid]);
    }

    fn session(user_id: UserId, created: DateTime, last_used: DateTime) -> InsertSession {
        let (mut session, _token) =
            InsertSession::new(user_id, SessionMethod::Email, None, created).unwrap();
        session.last_used = last_used;
        session
    }

    fn seed_user(conn: &mut diesel::SqliteConnection) -> UserId {
        diesel::insert_into(schema::user::table)
            .values((
                schema::user::uuid.eq("00000000-0000-4000-8000-000000000001"),
                schema::user::name.eq("User"),
                schema::user::slug.eq("user"),
                schema::user::email.eq("user@example.com"),
                schema::user::admin.eq(false),
                schema::user::locked.eq(false),
                schema::user::created.eq(0i64),
                schema::user::modified.eq(0i64),
            ))
            .execute(conn)
            .unwrap();
        schema::user::table
            .select(schema::user::id)
            .first(conn)
            .unwrap()
    }
}
