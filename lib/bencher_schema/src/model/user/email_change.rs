//! A user's asked-for change of email: one row per user, bound to the session that asked, which
//! its email's link names by a secret the row keeps only the hash of. It works once, for 30
//! minutes.

use bencher_json::{DateTime, Email, EmailChangeSecret, EmailChangeSecretHash};
use chrono::TimeDelta;
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{
    QueryUser, UserId,
    session::{QuerySession, SessionId},
    token::QueryToken,
};
use crate::{
    context::DbConnection,
    error::issue_error,
    schema::{self, email_change as email_change_table},
};

crate::macros::typed_id::typed_id!(EmailChangeId);

/// An email change's link stops working this long after it was asked for.
pub const EMAIL_CHANGE_LIFETIME: TimeDelta = TimeDelta::minutes(30);

#[derive(Debug, Clone, diesel::Queryable)]
#[diesel(table_name = email_change_table)]
pub struct QueryEmailChange {
    pub id: EmailChangeId,
    pub user_id: UserId,
    /// The session that asked, which the confirm keeps, unless it has ended.
    pub session_id: Option<SessionId>,
    /// The user's email when the change was asked for, the only one it replaces.
    pub email: Email,
    pub new_email: Email,
    pub secret_hash: EmailChangeSecretHash,
    pub created: DateTime,
    pub expiration: DateTime,
    pub confirmed: Option<DateTime>,
}

impl QueryEmailChange {
    /// What the secret names at `now`, as a confirm finds it.
    pub fn find(
        conn: &mut DbConnection,
        secret: &EmailChangeSecret,
        now: DateTime,
    ) -> diesel::QueryResult<Found> {
        let found = schema::email_change::table
            .inner_join(schema::user::table)
            .filter(schema::email_change::secret_hash.eq(EmailChangeSecretHash::from(secret)))
            .select((schema::email_change::all_columns, schema::user::all_columns))
            .first::<(Self, QueryUser)>(conn)
            .optional()?;
        Ok(match found {
            Some((change, _)) if change.expiration.timestamp() <= now.timestamp() => Found::Expired,
            Some((change, _)) if change.confirmed.is_some() => Found::Confirmed,
            // An email that moved since the ask is no longer the one the change replaces.
            Some((change, user)) if change.email == user.email => {
                Found::Live(Box::new(change), Box::new(user))
            },
            Some(_) | None => Found::Expired,
        })
    }

    /// Confirm the change the secret names, on one connection inside a write transaction.
    ///
    /// The user's email becomes the new one, proven at `now`; their legacy tokens are revoked;
    /// every session of theirs ends but the one that asked; and the row stays, confirmed, until it
    /// expires. A refusal changes nothing.
    pub fn confirm(
        conn: &mut DbConnection,
        secret: &EmailChangeSecret,
        now: DateTime,
    ) -> diesel::QueryResult<Confirmation> {
        let (change, user) = match Self::find(conn, secret, now)? {
            Found::Live(change, user) => (change, user),
            Found::Confirmed => return Ok(Confirmation::Used),
            Found::Expired => return Ok(Confirmation::Expired),
        };
        if user.locked {
            return Ok(Confirmation::Locked);
        }
        let taken = diesel::select(diesel::dsl::exists(
            schema::user::table.filter(schema::user::email.eq(&change.new_email)),
        ))
        .get_result::<bool>(conn)?;
        if taken {
            return Ok(Confirmation::Taken(change.new_email));
        }

        diesel::update(schema::user::table.filter(schema::user::id.eq(user.id)))
            .set((
                schema::user::email.eq(&change.new_email),
                schema::user::email_verified.eq(now),
                schema::user::modified.eq(now),
            ))
            .execute(conn)?;
        diesel::update(schema::email_change::table.filter(schema::email_change::id.eq(change.id)))
            .set(schema::email_change::confirmed.eq(now))
            .execute(conn)?;
        QueryToken::revoke_all(conn, user.id, now)?;
        if let Some(session_id) = change.session_id {
            QuerySession::delete_all_but(conn, user.id, session_id)?;
        } else {
            QuerySession::delete_all(conn, user.id)?;
        }
        let user = schema::user::table
            .filter(schema::user::id.eq(user.id))
            .first(conn)?;
        Ok(Confirmation::Confirmed(change, Box::new(user)))
    }

    /// End the user's change, if any, so its link no longer works, as a lock ends every credential.
    pub fn end(conn: &mut DbConnection, user_id: UserId) -> diesel::QueryResult<usize> {
        diesel::delete(
            schema::email_change::table.filter(schema::email_change::user_id.eq(user_id)),
        )
        .execute(conn)
    }
}

/// What an email change's secret names.
#[derive(Debug)]
pub enum Found {
    /// The change waits for its confirm, and the user's email is still the one it replaces.
    Live(Box<QueryEmailChange>, Box<QueryUser>),
    /// The change was confirmed and has not yet expired.
    Confirmed,
    /// The change expired, was asked for again, or never was, or the user's email moved since.
    Expired,
}

/// What confirming an email change did.
#[derive(Debug)]
pub enum Confirmation {
    /// The change, and its user with their new email.
    Confirmed(Box<QueryEmailChange>, Box<QueryUser>),
    Used,
    Expired,
    Locked,
    /// Another user has the new email.
    Taken(Email),
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = email_change_table)]
pub struct InsertEmailChange {
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub email: Email,
    pub new_email: Email,
    pub secret_hash: EmailChangeSecretHash,
    pub created: DateTime,
    pub expiration: DateTime,
    pub confirmed: Option<DateTime>,
}

impl InsertEmailChange {
    /// A change of the user's `email` to `new_email`, asked for from `session_id` at `now`, and
    /// the secret its link carries, which no row holds.
    pub fn new(
        user_id: UserId,
        session_id: Option<SessionId>,
        email: Email,
        new_email: Email,
        now: DateTime,
    ) -> Result<(Self, EmailChangeSecret), HttpError> {
        let secret = EmailChangeSecret::generate().map_err(|e| {
            issue_error(
                "Failed to generate an email change secret",
                "Failed to generate an email change secret:",
                e,
            )
        })?;
        let change = Self {
            user_id,
            session_id,
            email,
            new_email,
            secret_hash: EmailChangeSecretHash::from(&secret),
            created: now,
            expiration: now + EMAIL_CHANGE_LIFETIME,
            confirmed: None,
        };
        Ok((change, secret))
    }

    /// Record the change as the user's one change, renewing any earlier one, so its link no
    /// longer works, after dropping every change that has expired.
    pub fn upsert(&self, conn: &mut DbConnection) -> diesel::QueryResult<QueryEmailChange> {
        diesel::delete(
            schema::email_change::table.filter(schema::email_change::expiration.le(self.created)),
        )
        .execute(conn)?;
        diesel::insert_into(schema::email_change::table)
            .values(self)
            .on_conflict(schema::email_change::user_id)
            .do_update()
            .set((
                schema::email_change::session_id.eq(self.session_id),
                schema::email_change::email.eq(&self.email),
                schema::email_change::new_email.eq(&self.new_email),
                schema::email_change::secret_hash.eq(&self.secret_hash),
                schema::email_change::created.eq(self.created),
                schema::email_change::expiration.eq(self.expiration),
                schema::email_change::confirmed.eq(self.confirmed),
            ))
            .execute(conn)?;
        schema::email_change::table
            .filter(schema::email_change::user_id.eq(self.user_id))
            .first(conn)
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, EmailChangeSecret, EmailChangeSecretHash, SessionMethod};
    use chrono::TimeDelta;
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        connection::SimpleConnection as _,
        result::{DatabaseErrorKind, Error as DieselError},
    };
    use diesel_migrations::MigrationHarness as _;
    use pretty_assertions::assert_eq;

    use super::{Confirmation, EMAIL_CHANGE_LIFETIME, Found, InsertEmailChange, QueryEmailChange};
    use crate::{
        context::DbConnection,
        model::user::{
            QueryUser, UserId,
            session::{InsertSession, SessionId},
        },
        schema,
        test_util::setup_test_db,
    };

    const ADA: &str = "ada@example.com";
    const NEW: &str = "ada@lovelace.dev";
    const GRACE: &str = "grace@example.com";

    /// Ada, with a legacy token, and Grace.
    fn seed(conn: &mut DbConnection) -> (UserId, UserId) {
        conn.batch_execute(&format!(
            "INSERT INTO user (uuid, name, slug, email, admin, locked, created, modified)
                VALUES ('00000000-0000-4000-8000-000000000001', 'Ada', 'ada', '{ADA}', 0, 0, 0, 0),
                    ('00000000-0000-4000-8000-000000000002', 'Grace', 'grace', '{GRACE}', 0, 0, 0,
                    0);
            INSERT INTO token (uuid, user_id, name, jwt_hash, creation, expiration)
                SELECT '00000000-0000-4000-a000-000000000001', id, 'Legacy', '{hash}', 0,
                    4102444800
                FROM user WHERE email = '{ADA}';",
            hash = "0".repeat(64),
        ))
        .unwrap();
        (user(conn, ADA).id, user(conn, GRACE).id)
    }

    fn user(conn: &mut DbConnection, email: &str) -> QueryUser {
        QueryUser::find_with_email(conn, &email.parse().unwrap())
            .unwrap()
            .unwrap()
    }

    fn session(conn: &mut DbConnection, user_id: UserId, now: DateTime) -> SessionId {
        let (session, _token) =
            InsertSession::new(user_id, SessionMethod::Email, None, now).unwrap();
        session.insert(conn, now).unwrap();
        schema::session::table
            .filter(schema::session::uuid.eq(session.uuid))
            .select(schema::session::id)
            .first(conn)
            .unwrap()
    }

    fn sessions(conn: &mut DbConnection) -> Vec<SessionId> {
        schema::session::table
            .select(schema::session::id)
            .order(schema::session::id)
            .load(conn)
            .unwrap()
    }

    fn revoked(conn: &mut DbConnection) -> Vec<Option<i64>> {
        schema::token::table
            .select(schema::token::revoked)
            .load(conn)
            .unwrap()
    }

    fn confirmed(conn: &mut DbConnection) -> Option<DateTime> {
        schema::email_change::table
            .select(schema::email_change::confirmed)
            .first(conn)
            .unwrap()
    }

    fn ask(
        conn: &mut DbConnection,
        user_id: UserId,
        session_id: Option<SessionId>,
        now: DateTime,
    ) -> (QueryEmailChange, EmailChangeSecret) {
        let (insert, secret) = InsertEmailChange::new(
            user_id,
            session_id,
            ADA.parse().unwrap(),
            NEW.parse().unwrap(),
            now,
        )
        .unwrap();
        (insert.upsert(conn).unwrap(), secret)
    }

    // The row keeps the asking session and the secret's hash alone, and the secret finds it until
    // its last second; a secret no row holds reads as expired.
    #[test]
    fn ask_keeps_only_the_secrets_hash_and_lives_thirty_minutes() {
        let mut conn = setup_test_db();
        let (ada, _grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let asking = session(&mut conn, ada, now);
        let (change, secret) = ask(&mut conn, ada, Some(asking), now);

        assert_eq!(change.session_id, Some(asking));
        assert_eq!(change.secret_hash, EmailChangeSecretHash::from(&secret));
        assert_eq!(change.expiration, now + EMAIL_CHANGE_LIFETIME);
        assert_eq!(change.confirmed, None);
        let stored = schema::email_change::table
            .select(schema::email_change::secret_hash)
            .load::<String>(&mut conn)
            .unwrap();
        assert!(
            stored.iter().all(|hash| hash != secret.as_ref()),
            "{stored:?}"
        );

        let last = change.expiration - TimeDelta::seconds(1);
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &secret, last).unwrap(),
            Found::Live(..)
        ));
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &secret, change.expiration).unwrap(),
            Found::Expired
        ));
        let unknown = EmailChangeSecret::generate().unwrap();
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &unknown, now).unwrap(),
            Found::Expired
        ));
    }

    // A second ask keeps one row per user, renewed with a new secret, a new life, and the session
    // that asked again, so the first link finds nothing, even once it was confirmed.
    #[test]
    fn ask_again_renews_the_row_and_ends_the_earlier_link() {
        let mut conn = setup_test_db();
        let (ada, _grace) = seed(&mut conn);
        let first_at = DateTime::TEST;
        let first_session = session(&mut conn, ada, first_at);
        let (first, first_secret) = ask(&mut conn, ada, Some(first_session), first_at);
        diesel::update(schema::email_change::table)
            .set(schema::email_change::confirmed.eq(first_at))
            .execute(&mut conn)
            .unwrap();

        let again_at = first_at + TimeDelta::minutes(10);
        let again_session = session(&mut conn, ada, again_at);
        let (again, again_secret) = ask(&mut conn, ada, Some(again_session), again_at);

        assert_eq!(again.id, first.id);
        assert_eq!(again.session_id, Some(again_session));
        assert_eq!(again.expiration, again_at + EMAIL_CHANGE_LIFETIME);
        assert_eq!(again.confirmed, None);
        assert_eq!(
            schema::email_change::table
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap(),
            1
        );
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &first_secret, again_at).unwrap(),
            Found::Expired
        ));
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &again_secret, again_at).unwrap(),
            Found::Live(..)
        ));
    }

    // An ask drops every change that has expired, of any user, and leaves the live ones.
    // Kills an ask that keeps expired changes, or drops a live one.
    #[test]
    fn ask_drops_every_expired_change() {
        let mut conn = setup_test_db();
        let (ada, grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let (grace_change, _secret) = InsertEmailChange::new(
            grace,
            None,
            GRACE.parse().unwrap(),
            "grace@hopper.dev".parse().unwrap(),
            now,
        )
        .unwrap();
        grace_change.upsert(&mut conn).unwrap();
        let users = |conn: &mut DbConnection| {
            schema::email_change::table
                .select(schema::email_change::user_id)
                .order(schema::email_change::user_id)
                .load::<UserId>(conn)
                .unwrap()
        };

        let _ada = ask(&mut conn, ada, None, now);
        assert_eq!(users(&mut conn), vec![ada, grace]);

        let later = now + EMAIL_CHANGE_LIFETIME;
        let _again = ask(&mut conn, ada, None, later);
        assert_eq!(users(&mut conn), vec![ada]);
    }

    // Confirming switches the address, proves it, marks the user modified, revokes the user's legacy
    // tokens, marks the row, and ends every session of the user but the one that asked, once.
    // Kills a confirm that keeps another session, ends the asking one or another user's, leaves a
    // token, leaves the user or the row unmarked, or works twice.
    #[test]
    fn confirm_switches_once_and_keeps_only_the_asking_session() {
        let mut conn = setup_test_db();
        let (ada, grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let _other = session(&mut conn, ada, now);
        let asking = session(&mut conn, ada, now);
        let graces = session(&mut conn, grace, now);
        let (_change, secret) = ask(&mut conn, ada, Some(asking), now);
        let at = now + TimeDelta::minutes(5);

        let Confirmation::Confirmed(_change, confirmed_user) =
            QueryEmailChange::confirm(&mut conn, &secret, at).unwrap()
        else {
            panic!("The change was not confirmed");
        };
        assert_eq!(confirmed_user.id, ada);
        assert_eq!(confirmed_user.email.as_ref(), NEW);
        assert_eq!(confirmed_user.email_verified, Some(at));
        assert_eq!(confirmed_user.modified, at);
        assert_eq!(revoked(&mut conn), vec![Some(at.timestamp())]);
        assert_eq!(sessions(&mut conn), vec![asking, graces]);
        assert_eq!(confirmed(&mut conn), Some(at));

        assert!(matches!(
            QueryEmailChange::find(&mut conn, &secret, at).unwrap(),
            Found::Confirmed
        ));
        assert!(matches!(
            QueryEmailChange::confirm(&mut conn, &secret, at).unwrap(),
            Confirmation::Used
        ));
        assert_eq!(confirmed(&mut conn), Some(at));
    }

    // Once the session that asked has ended, no session of the user is kept.
    #[test]
    fn confirm_after_the_asking_session_ended_ends_every_session() {
        let mut conn = setup_test_db();
        let (ada, grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let asking = session(&mut conn, ada, now);
        let _other = session(&mut conn, ada, now);
        let graces = session(&mut conn, grace, now);
        let (_change, secret) = ask(&mut conn, ada, Some(asking), now);
        diesel::delete(schema::session::table.filter(schema::session::id.eq(asking)))
            .execute(&mut conn)
            .unwrap();

        assert!(matches!(
            QueryEmailChange::confirm(&mut conn, &secret, now).unwrap(),
            Confirmation::Confirmed(..)
        ));
        assert_eq!(sessions(&mut conn), vec![graces]);
    }

    // An expired change, a renewed change's earlier link, a user whose email moved since the ask, a
    // locked user, and a new email another user took since each refuse and change nothing.
    #[test]
    fn confirm_refuses_and_changes_nothing() {
        let now = DateTime::TEST;
        for case in ["expired", "renewed", "moved", "locked", "taken"] {
            let mut conn = setup_test_db();
            let (ada, grace) = seed(&mut conn);
            let asking = session(&mut conn, ada, now);
            let _other = session(&mut conn, ada, now);
            let (_change, secret) = ask(&mut conn, ada, Some(asking), now);
            let at = now + TimeDelta::minutes(1);
            match case {
                "expired" => {
                    diesel::update(schema::email_change::table)
                        .set(schema::email_change::expiration.eq(at))
                        .execute(&mut conn)
                        .unwrap();
                },
                "renewed" => {
                    let _renewed = ask(&mut conn, ada, Some(asking), now);
                },
                "moved" => {
                    diesel::update(schema::user::table.filter(schema::user::id.eq(ada)))
                        .set(schema::user::email.eq("ada@moved.example"))
                        .execute(&mut conn)
                        .unwrap();
                },
                "locked" => {
                    diesel::update(schema::user::table.filter(schema::user::id.eq(ada)))
                        .set(schema::user::locked.eq(true))
                        .execute(&mut conn)
                        .unwrap();
                },
                _ => {
                    diesel::update(schema::user::table.filter(schema::user::id.eq(grace)))
                        .set(schema::user::email.eq(NEW))
                        .execute(&mut conn)
                        .unwrap();
                },
            }
            let before = QueryUser::get(&mut conn, ada).unwrap();
            let sessions_before = sessions(&mut conn);

            let refused = QueryEmailChange::confirm(&mut conn, &secret, at).unwrap();
            let expected = match case {
                "expired" | "renewed" | "moved" => matches!(refused, Confirmation::Expired),
                "locked" => matches!(refused, Confirmation::Locked),
                _ => matches!(&refused, Confirmation::Taken(email) if email.as_ref() == NEW),
            };
            assert!(expected, "{case}: {refused:?}");
            let after = QueryUser::get(&mut conn, ada).unwrap();
            assert_eq!(
                (after.email, after.email_verified, after.modified),
                (before.email, before.email_verified, before.modified),
                "{case}"
            );
            assert_eq!(revoked(&mut conn), vec![None], "{case}");
            assert_eq!(sessions(&mut conn), sessions_before, "{case}");
            assert_eq!(confirmed(&mut conn), None, "{case}");
        }
    }

    // A secret names one change, so no second row takes its hash; a user has one row; the end of
    // the session that asked keeps the change without it; and a user who goes takes their change.
    // Kills a hash two changes may share, two rows for one user, a session whose end takes the
    // change or is refused, and a user whose delete leaves their change behind or is refused.
    #[test]
    fn change_names_one_row_by_its_hash_and_one_per_user() {
        let mut conn = setup_test_db();
        let (ada, grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let asking = session(&mut conn, ada, now);
        let (change, _secret) = ask(&mut conn, ada, Some(asking), now);
        let insert = |user_id, secret_hash| InsertEmailChange {
            user_id,
            session_id: None,
            email: GRACE.parse().unwrap(),
            new_email: "grace@hopper.dev".parse().unwrap(),
            secret_hash,
            created: now,
            expiration: now + EMAIL_CHANGE_LIFETIME,
            confirmed: None,
        };
        let another_hash = EmailChangeSecretHash::from(&EmailChangeSecret::generate().unwrap());
        for (case, row) in [
            ("shared hash", insert(grace, change.secret_hash)),
            ("second row", insert(ada, another_hash)),
        ] {
            let inserted = diesel::insert_into(schema::email_change::table)
                .values(&row)
                .execute(&mut conn);
            assert!(
                matches!(
                    inserted,
                    Err(DieselError::DatabaseError(
                        DatabaseErrorKind::UniqueViolation,
                        _
                    ))
                ),
                "{case}: {inserted:?}"
            );
        }

        diesel::delete(schema::session::table.filter(schema::session::id.eq(asking)))
            .execute(&mut conn)
            .unwrap();
        assert_eq!(
            schema::email_change::table
                .select(schema::email_change::session_id)
                .load::<Option<SessionId>>(&mut conn)
                .unwrap(),
            vec![None]
        );
        diesel::delete(schema::token::table)
            .execute(&mut conn)
            .unwrap();
        diesel::delete(schema::user::table.filter(schema::user::id.eq(ada)))
            .execute(&mut conn)
            .unwrap();
        assert_eq!(
            schema::email_change::table
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap(),
            0
        );
    }

    const EMAIL_CHANGE_MIGRATION: &str = "20261114120000";

    #[derive(diesel::QueryableByName)]
    struct Text {
        #[diesel(sql_type = diesel::sql_types::Text)]
        text: String,
    }

    // The migration adds the table, its columns in order, beside every row already there; the
    // down migration takes it and its rows away and leaves the rest; and it runs up again.
    #[test]
    fn email_change_migration_keeps_every_row() {
        let mut conn = setup_test_db();
        let (ada, _grace) = seed(&mut conn);
        let now = DateTime::TEST;
        let asking = session(&mut conn, ada, now);
        let rows = every_row(&mut conn);

        revert_to_before_the_migration(&mut conn);
        assert_eq!(tables(&mut conn, "email_change"), "0");
        assert_eq!(every_row(&mut conn), rows);

        crate::run_migrations(&mut conn).unwrap();
        assert_eq!(
            text(
                &mut conn,
                "SELECT group_concat(name, ', ') AS text FROM pragma_table_info('email_change')"
            ),
            "id, user_id, session_id, email, new_email, secret_hash, created, expiration, confirmed"
        );
        assert_eq!(every_row(&mut conn), rows);
        let (_change, secret) = ask(&mut conn, ada, Some(asking), now);

        revert_to_before_the_migration(&mut conn);
        assert_eq!(tables(&mut conn, "email_change"), "0");
        assert_eq!(every_row(&mut conn), rows);
        assert_eq!(
            text(
                &mut conn,
                "SELECT CAST(COUNT(*) AS TEXT) AS text FROM pragma_foreign_key_check"
            ),
            "0"
        );

        crate::run_migrations(&mut conn).unwrap();
        assert_eq!(every_row(&mut conn), rows);
        assert!(matches!(
            QueryEmailChange::find(&mut conn, &secret, now).unwrap(),
            Found::Expired
        ));
    }

    fn revert_to_before_the_migration(conn: &mut DbConnection) {
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == EMAIL_CHANGE_MIGRATION {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
    }

    fn tables(conn: &mut DbConnection, table: &str) -> String {
        text(
            conn,
            &format!(
                "SELECT CAST(COUNT(*) AS TEXT) AS text FROM sqlite_master \
                 WHERE type = 'table' AND name = '{table}'"
            ),
        )
    }

    /// Every user, session, and legacy token, as rows of text.
    fn every_row(conn: &mut DbConnection) -> String {
        text(
            conn,
            "SELECT group_concat(row, ';') AS text FROM (
                SELECT concat_ws('|', 'user', id, uuid, email, modified) AS row FROM user
                UNION ALL
                SELECT concat_ws('|', 'session', id, uuid, user_id, token_hash) FROM session
                UNION ALL
                SELECT concat_ws('|', 'token', id, uuid, user_id, revoked) FROM token
                ORDER BY row
            )",
        )
    }

    fn text(conn: &mut DbConnection, query: &str) -> String {
        diesel::sql_query(query)
            .get_result::<Text>(conn)
            .unwrap()
            .text
    }
}
