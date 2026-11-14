//! An invitation to an organization: a row bound to the address it invites, which its email's link
//! names by a secret the row keeps only the hash of. It works once, for 7 days.

use bencher_json::{
    DateTime, Email, InviteSecret, InviteSecretHash, InviteUuid, JsonInvite, JsonPubUser,
    organization::member::OrganizationRole,
};
use chrono::TimeDelta;
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{
    OrganizationId, QueryOrganization,
    organization_role::{InsertOrganizationRole, QueryOrganizationRole},
};
use crate::{
    context::DbConnection,
    error::{issue_error, resource_not_found_err},
    model::user::{QueryUser, UserId},
    schema::{self, invite as invite_table},
};

crate::macros::typed_id::typed_id!(InviteId);

/// An invitation stops working this long after it was sent.
pub const INVITE_LIFETIME: TimeDelta = TimeDelta::days(7);

#[derive(Debug, Clone, diesel::Queryable)]
#[diesel(table_name = invite_table)]
pub struct QueryInvite {
    pub id: InviteId,
    pub uuid: InviteUuid,
    pub organization_id: OrganizationId,
    pub email: Email,
    pub role: OrganizationRole,
    pub inviter_id: UserId,
    pub secret_hash: InviteSecretHash,
    pub created: DateTime,
    pub expiration: DateTime,
}

impl QueryInvite {
    /// The invitation `secret` names, with its organization, unless it has ended, has expired, or
    /// its organization is gone.
    pub fn from_secret(
        conn: &mut DbConnection,
        secret: &InviteSecret,
        now: DateTime,
    ) -> diesel::QueryResult<Option<(Self, QueryOrganization)>> {
        schema::invite::table
            .inner_join(schema::organization::table)
            .filter(schema::invite::secret_hash.eq(InviteSecretHash::from(secret)))
            .filter(schema::invite::expiration.gt(now))
            .filter(schema::organization::deleted.is_null())
            .select((
                schema::invite::all_columns,
                schema::organization::all_columns,
            ))
            .first(conn)
            .optional()
    }

    /// The invitation with `id`, as [`Self::from_secret`] finds one.
    pub fn find(
        conn: &mut DbConnection,
        id: InviteId,
        now: DateTime,
    ) -> diesel::QueryResult<Option<(Self, QueryOrganization)>> {
        schema::invite::table
            .inner_join(schema::organization::table)
            .filter(schema::invite::id.eq(id))
            .filter(schema::invite::expiration.gt(now))
            .filter(schema::organization::deleted.is_null())
            .select((
                schema::invite::all_columns,
                schema::organization::all_columns,
            ))
            .first(conn)
            .optional()
    }

    /// The organization's invitation with `uuid`, expired or not, unless it has ended.
    pub fn from_uuid(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        uuid: InviteUuid,
    ) -> Result<Self, HttpError> {
        schema::invite::table
            .filter(schema::invite::organization_id.eq(organization_id))
            .filter(schema::invite::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Invite, (organization_id, uuid)))
    }

    /// Accept the invitation for `user`, on one connection inside a write transaction.
    ///
    /// An invitation for another address is left as it is. Otherwise it ends, so it works once,
    /// and `user` joins its organization with its role, unless they already belong to it.
    pub fn accept(
        &self,
        conn: &mut DbConnection,
        user: &QueryUser,
        now: DateTime,
    ) -> diesel::QueryResult<Accepted> {
        if self.email != user.email {
            return Ok(Accepted::ForAnotherEmail);
        }
        if Self::end(conn, self.id)? == 0 {
            return Ok(Accepted::Ended);
        }
        if QueryOrganizationRole::exists(conn, user.id, self.organization_id)? {
            return Ok(Accepted::AlreadyMember);
        }
        diesel::insert_into(schema::organization_role::table)
            .values(&InsertOrganizationRole {
                user_id: user.id,
                organization_id: self.organization_id,
                role: self.role,
                created: now,
                modified: now,
            })
            .execute(conn)?;
        Ok(Accepted::Joined)
    }

    /// End the invitation, so its link starts nothing and a sign-in it started joins nothing.
    pub fn end(conn: &mut DbConnection, id: InviteId) -> diesel::QueryResult<usize> {
        diesel::delete(schema::invite::table.filter(schema::invite::id.eq(id))).execute(conn)
    }

    /// Send the invitation again, from `inviter_id`, with a new secret that works for
    /// [`INVITE_LIFETIME`] from `now`, so its earlier link starts nothing new.
    pub fn renew(
        &self,
        conn: &mut DbConnection,
        inviter_id: UserId,
        now: DateTime,
    ) -> Result<(Self, InviteSecret), HttpError> {
        let secret = generate_secret()?;
        let renew = |conn: &mut DbConnection| {
            diesel::update(schema::invite::table.filter(schema::invite::id.eq(self.id)))
                .set((
                    schema::invite::inviter_id.eq(inviter_id),
                    schema::invite::secret_hash.eq(InviteSecretHash::from(&secret)),
                    schema::invite::created.eq(now),
                    schema::invite::expiration.eq(now + INVITE_LIFETIME),
                ))
                .execute(conn)?;
            schema::invite::table
                .filter(schema::invite::id.eq(self.id))
                .first(conn)
        };
        let renewed = renew(conn).map_err(|e| {
            issue_error(
                "Failed to renew an invitation",
                "Failed to renew an invitation:",
                e,
            )
        })?;
        Ok((renewed, secret))
    }

    pub fn into_json(self, inviter: JsonPubUser) -> JsonInvite {
        let Self {
            uuid,
            email,
            role,
            created,
            expiration,
            ..
        } = self;
        JsonInvite {
            uuid,
            email,
            role,
            inviter,
            created,
            expiration,
        }
    }
}

/// What accepting an invitation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accepted {
    Joined,
    /// The invitation ended, and the user already belonged to its organization.
    AlreadyMember,
    /// The invitation is for another address, and it is left as it is.
    ForAnotherEmail,
    /// The invitation ended before this could end it.
    Ended,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = invite_table)]
pub struct InsertInvite {
    pub uuid: InviteUuid,
    pub organization_id: OrganizationId,
    pub email: Email,
    pub role: OrganizationRole,
    pub inviter_id: UserId,
    pub secret_hash: InviteSecretHash,
    pub created: DateTime,
    pub expiration: DateTime,
}

impl InsertInvite {
    /// Invite `email` to the organization as `role`, from `inviter_id`, on the writer, and return
    /// the invitation and the secret its link carries.
    ///
    /// An address the organization already invited is invited again in the same row, which keeps
    /// its ID, so a sign-in its earlier link started still joins, while that link starts nothing
    /// new.
    pub fn invite(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        email: Email,
        role: OrganizationRole,
        inviter_id: UserId,
        now: DateTime,
    ) -> Result<(QueryInvite, InviteSecret), HttpError> {
        let secret = generate_secret()?;
        let insert = Self {
            uuid: InviteUuid::new(),
            organization_id,
            email,
            role,
            inviter_id,
            secret_hash: InviteSecretHash::from(&secret),
            created: now,
            expiration: now + INVITE_LIFETIME,
        };
        let upsert = |conn: &mut DbConnection| {
            diesel::insert_into(schema::invite::table)
                .values(&insert)
                .on_conflict((schema::invite::organization_id, schema::invite::email))
                .do_update()
                .set((
                    schema::invite::role.eq(insert.role),
                    schema::invite::inviter_id.eq(insert.inviter_id),
                    schema::invite::secret_hash.eq(&insert.secret_hash),
                    schema::invite::created.eq(insert.created),
                    schema::invite::expiration.eq(insert.expiration),
                ))
                .execute(conn)?;
            schema::invite::table
                .filter(schema::invite::organization_id.eq(insert.organization_id))
                .filter(schema::invite::email.eq(&insert.email))
                .first(conn)
        };
        let invite = upsert(conn)
            .map_err(|e| issue_error("Failed to invite", "Failed to store an invitation:", e))?;
        Ok((invite, secret))
    }
}

fn generate_secret() -> Result<InviteSecret, HttpError> {
    InviteSecret::generate().map_err(|e| {
        issue_error(
            "Failed to generate an invitation secret",
            "Failed to generate an invitation secret:",
            e,
        )
    })
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, InviteSecretHash, InviteUuid, OrganizationUuid, UserUuid,
        organization::member::OrganizationRole,
    };
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        connection::SimpleConnection as _,
        result::{DatabaseErrorKind, Error as DieselError},
    };
    use diesel_migrations::MigrationHarness as _;
    use pretty_assertions::assert_eq;

    use super::{Accepted, INVITE_LIFETIME, InsertInvite, QueryInvite};
    use crate::{
        context::DbConnection,
        model::{
            organization::OrganizationId,
            user::{QueryUser, UserId},
        },
        schema,
        test_util::setup_test_db,
    };

    const LEADER: &str = "ada@example.com";
    const INVITED: &str = "grace@example.com";

    struct Seeded {
        organization: OrganizationId,
        leader: UserId,
    }

    fn seed(conn: &mut DbConnection) -> Seeded {
        conn.batch_execute(&format!(
            "INSERT INTO organization (uuid, name, slug, created, modified)
                VALUES ('{organization}', 'Organization', 'organization', 0, 0);
            INSERT INTO user (uuid, name, slug, email, admin, locked, created, modified)
                VALUES ('{leader}', 'Ada', 'ada', '{LEADER}', 0, 0, 0, 0),
                    ('{invited}', 'Grace', 'grace', '{INVITED}', 0, 0, 0, 0);",
            organization = OrganizationUuid::new(),
            leader = UserUuid::new(),
            invited = UserUuid::new(),
        ))
        .unwrap();
        Seeded {
            organization: schema::organization::table
                .select(schema::organization::id)
                .first(conn)
                .unwrap(),
            leader: user(conn, LEADER).id,
        }
    }

    fn user(conn: &mut DbConnection, email: &str) -> QueryUser {
        QueryUser::find_with_email(conn, &email.parse().unwrap())
            .unwrap()
            .unwrap()
    }

    fn invite(conn: &mut DbConnection, seeded: &Seeded, now: DateTime) -> QueryInvite {
        let (invite, _secret) = InsertInvite::invite(
            conn,
            seeded.organization,
            INVITED.parse().unwrap(),
            OrganizationRole::Leader,
            seeded.leader,
            now,
        )
        .unwrap();
        invite
    }

    fn roles(conn: &mut DbConnection) -> i64 {
        schema::organization_role::table
            .count()
            .get_result(conn)
            .unwrap()
    }

    // The row keeps the secret's hash alone, and the secret finds it until its last second.
    #[test]
    fn invite_keeps_only_its_secrets_hash_and_lives_seven_days() {
        let mut conn = setup_test_db();
        let seeded = seed(&mut conn);
        let now = DateTime::TEST;
        let (invite, secret) = InsertInvite::invite(
            &mut conn,
            seeded.organization,
            INVITED.parse().unwrap(),
            OrganizationRole::Leader,
            seeded.leader,
            now,
        )
        .unwrap();
        assert_eq!(invite.secret_hash, InviteSecretHash::from(&secret));
        assert_eq!(invite.expiration, now + INVITE_LIFETIME);
        let stored = schema::invite::table
            .select(schema::invite::secret_hash)
            .load::<String>(&mut conn)
            .unwrap();
        assert!(
            stored.iter().all(|hash| hash != secret.as_ref()),
            "{stored:?}"
        );

        let last = invite.expiration - chrono::TimeDelta::seconds(1);
        for (at, found) in [(last, true), (invite.expiration, false)] {
            assert_eq!(
                QueryInvite::from_secret(&mut conn, &secret, at)
                    .unwrap()
                    .is_some(),
                found,
                "{at}"
            );
            assert_eq!(
                QueryInvite::find(&mut conn, invite.id, at)
                    .unwrap()
                    .is_some(),
                found,
                "{at}"
            );
        }
    }

    // A second invitation to the address keeps the row and its ID, so a sign-in the first started
    // still finds it, and takes a new secret, a new life, and its inviter, so the first link
    // finds nothing.
    #[test]
    fn invite_again_renews_the_row() {
        let mut conn = setup_test_db();
        let seeded = seed(&mut conn);
        let first_at = DateTime::TEST;
        let (first, first_secret) = InsertInvite::invite(
            &mut conn,
            seeded.organization,
            INVITED.parse().unwrap(),
            OrganizationRole::Leader,
            seeded.leader,
            first_at,
        )
        .unwrap();
        let invited = user(&mut conn, INVITED).id;
        let again_at = first_at + chrono::TimeDelta::days(3);
        let (again, again_secret) = InsertInvite::invite(
            &mut conn,
            seeded.organization,
            INVITED.parse().unwrap(),
            OrganizationRole::Leader,
            invited,
            again_at,
        )
        .unwrap();

        assert_eq!((again.id, again.uuid), (first.id, first.uuid));
        assert_eq!(again.inviter_id, invited);
        assert_eq!(again.expiration, again_at + INVITE_LIFETIME);
        assert_eq!(
            schema::invite::table
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap(),
            1
        );
        assert!(
            QueryInvite::from_secret(&mut conn, &first_secret, again_at)
                .unwrap()
                .is_none()
        );
        assert!(
            QueryInvite::from_secret(&mut conn, &again_secret, again_at)
                .unwrap()
                .is_some()
        );
    }

    // Accepting ends the row and joins with its role, once; an invitation for another address
    // changes nothing; and a member's acceptance ends the row and joins nothing.
    #[test]
    fn accept_joins_once_and_only_its_address() {
        let mut conn = setup_test_db();
        let seeded = seed(&mut conn);
        let now = DateTime::TEST;
        let invite = invite(&mut conn, &seeded, now);
        let leader = user(&mut conn, LEADER);
        let invited = user(&mut conn, INVITED);

        assert_eq!(
            invite.accept(&mut conn, &leader, now).unwrap(),
            Accepted::ForAnotherEmail
        );
        assert_eq!(roles(&mut conn), 0);
        assert!(
            QueryInvite::find(&mut conn, invite.id, now)
                .unwrap()
                .is_some()
        );

        assert_eq!(
            invite.accept(&mut conn, &invited, now).unwrap(),
            Accepted::Joined
        );
        assert_eq!(
            schema::organization_role::table
                .filter(schema::organization_role::user_id.eq(invited.id))
                .filter(schema::organization_role::organization_id.eq(seeded.organization))
                .select(schema::organization_role::role)
                .first::<OrganizationRole>(&mut conn)
                .unwrap(),
            OrganizationRole::Leader
        );
        assert!(
            QueryInvite::find(&mut conn, invite.id, now)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            invite.accept(&mut conn, &invited, now).unwrap(),
            Accepted::Ended
        );
        assert_eq!(roles(&mut conn), 1);

        let again = self::invite(&mut conn, &seeded, now);
        assert_eq!(
            again.accept(&mut conn, &invited, now).unwrap(),
            Accepted::AlreadyMember
        );
        assert_eq!(roles(&mut conn), 1);
        assert!(
            QueryInvite::find(&mut conn, again.id, now)
                .unwrap()
                .is_none()
        );
    }

    // An organization that is gone takes its invitations with it.
    #[test]
    fn invite_of_a_deleted_organization_is_not_found() {
        let mut conn = setup_test_db();
        let seeded = seed(&mut conn);
        let now = DateTime::TEST;
        let (invite, secret) = InsertInvite::invite(
            &mut conn,
            seeded.organization,
            INVITED.parse().unwrap(),
            OrganizationRole::Leader,
            seeded.leader,
            now,
        )
        .unwrap();
        diesel::update(
            schema::organization::table.filter(schema::organization::id.eq(seeded.organization)),
        )
        .set(schema::organization::deleted.eq(now))
        .execute(&mut conn)
        .unwrap();
        assert!(
            QueryInvite::from_secret(&mut conn, &secret, now)
                .unwrap()
                .is_none()
        );
        assert!(
            QueryInvite::find(&mut conn, invite.id, now)
                .unwrap()
                .is_none()
        );
    }

    // A secret names one invitation, so no second row takes its hash; and an account that goes
    // takes the invitations it sent.
    // Kills a hash two invitations may share, and an inviter whose delete is refused while an
    // invitation of theirs waits, or leaves it behind.
    #[test]
    fn invite_names_one_row_by_its_hash_and_goes_with_its_inviter() {
        let mut conn = setup_test_db();
        let seeded = seed(&mut conn);
        let now = DateTime::TEST;
        let invite = invite(&mut conn, &seeded, now);
        let shared = diesel::insert_into(schema::invite::table)
            .values(&InsertInvite {
                uuid: InviteUuid::new(),
                organization_id: seeded.organization,
                email: LEADER.parse().unwrap(),
                role: OrganizationRole::Leader,
                inviter_id: seeded.leader,
                secret_hash: invite.secret_hash,
                created: now,
                expiration: now + INVITE_LIFETIME,
            })
            .execute(&mut conn);
        assert!(
            matches!(
                shared,
                Err(DieselError::DatabaseError(
                    DatabaseErrorKind::UniqueViolation,
                    _
                ))
            ),
            "{shared:?}"
        );

        diesel::delete(schema::user::table.filter(schema::user::id.eq(seeded.leader)))
            .execute(&mut conn)
            .unwrap();
        assert_eq!(
            schema::invite::table
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap(),
            0
        );
    }

    const INVITE_MIGRATION: &str = "20261113120000";
    /// Every column a sign-in table keeps through the migration, in order, by table.
    const KEPT: [(&str, &str); 2] = [
        (
            "pending_sign_in",
            "id, uuid, session_id, email, binding_hash, code_hash, link_hash, tries, claim, plan, \
             authorization, pin_provider, pin_account, pin_handle, created, expiration",
        ),
        (
            "provider_state",
            "id, session_id, provider, state_hash, binding_hash, verifier, nonce, claim, plan, \
             authorization, created, expiration",
        ),
    ];

    #[derive(diesel::QueryableByName)]
    struct Text {
        #[diesel(sql_type = diesel::sql_types::Text)]
        text: String,
    }

    // The migration rebuilds both sign-in tables with an invitation's ID in its token's place: every
    // row stays, each kept column as it was, with no invitation, since no token names a row; and
    // the down migration gives back the token's column with every row.
    #[test]
    fn invite_migration_keeps_every_sign_in() {
        let mut conn = setup_test_db();
        revert_to_before_the_migration(&mut conn);
        seed_sign_ins(&mut conn);
        let rows = kept_rows(&mut conn);

        crate::run_migrations(&mut conn).unwrap();
        assert_eq!(
            columns(&mut conn, "pending_sign_in"),
            "id, uuid, session_id, email, binding_hash, code_hash, link_hash, tries, invite_id, \
             claim, plan, authorization, pin_provider, pin_account, pin_handle, created, expiration"
        );
        assert_eq!(
            columns(&mut conn, "provider_state"),
            "id, session_id, provider, state_hash, binding_hash, verifier, nonce, invite_id, claim, \
             plan, authorization, created, expiration"
        );
        assert_eq!(kept_rows(&mut conn), rows);
        for (table, _columns) in KEPT {
            assert_eq!(
                text(
                    &mut conn,
                    &format!(
                        "SELECT CAST(COUNT(*) AS TEXT) AS text FROM {table} WHERE invite_id IS NOT NULL"
                    )
                ),
                "0",
                "{table}"
            );
        }
        assert_eq!(
            text(
                &mut conn,
                "SELECT CAST(COUNT(*) AS TEXT) AS text FROM pragma_foreign_key_check"
            ),
            "0"
        );

        revert_to_before_the_migration(&mut conn);
        assert_eq!(
            columns(&mut conn, "pending_sign_in"),
            "id, uuid, session_id, email, binding_hash, code_hash, link_hash, tries, invite, claim, \
             plan, authorization, pin_provider, pin_account, pin_handle, created, expiration"
        );
        assert_eq!(
            columns(&mut conn, "provider_state"),
            "id, session_id, provider, state_hash, binding_hash, verifier, nonce, invite, claim, \
             plan, authorization, created, expiration"
        );
        assert_eq!(kept_rows(&mut conn), rows);
    }

    fn revert_to_before_the_migration(conn: &mut DbConnection) {
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == INVITE_MIGRATION {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
    }

    /// Sign-ins by email and with a provider, each of a session and of none, with an invitation's
    /// token and without one, in the shape the tables had before the migration.
    fn seed_sign_ins(conn: &mut DbConnection) {
        conn.batch_execute(
            "INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                VALUES (1, '00000000-0000-4000-8000-000000000001', 'Ada', 'ada',
                    'ada@example.com', 0, 0, 1700000000, 1700000000);
            INSERT INTO session (id, uuid, token_hash, user_id, method, device, created, proved,
                    last_used)
                VALUES (1, '00000000-0000-4000-8000-0000000000a1', 'session-hash', 1, 0, NULL,
                    1700000000, 1700000000, 1700000000);
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 64)
            INSERT INTO pending_sign_in (id, uuid, session_id, email, binding_hash, code_hash,
                    link_hash, tries, invite, claim, plan, authorization, pin_provider,
                    pin_account, pin_handle, created, expiration)
                SELECT i, printf('00000000-0000-4000-9000-%012d', i),
                    CASE WHEN i % 4 = 0 THEN 1 END, 'user' || i || '@example.com',
                    'binding-' || i, 'code-' || i, 'link-' || i, i % 5,
                    CASE WHEN i % 2 = 0 THEN 'invite.jwt.' || i END,
                    CASE WHEN i % 3 = 0 THEN printf('00000000-0000-4000-b000-%012d', i) END,
                    CASE WHEN i % 5 = 0 THEN 'pro' END, NULL,
                    CASE WHEN i % 7 = 0 THEN 0 END, CASE WHEN i % 7 = 0 THEN 'account-' || i END,
                    CASE WHEN i % 7 = 0 THEN 'handle-' || i END, 1700000000 + i, 1700001800 + i
                FROM n;
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 64)
            INSERT INTO provider_state (id, session_id, provider, state_hash, binding_hash,
                    verifier, nonce, invite, claim, plan, authorization, created, expiration)
                SELECT i, CASE WHEN i % 4 = 0 THEN 1 END, i % 2, 'state-' || i, 'binding-' || i,
                    'verifier-' || i, CASE WHEN i % 2 = 1 THEN 'nonce-' || i END,
                    CASE WHEN i % 2 = 0 THEN 'invite.jwt.' || i END,
                    CASE WHEN i % 3 = 0 THEN printf('00000000-0000-4000-c000-%012d', i) END,
                    NULL, NULL, 1700000000 + i, 1700000600 + i
                FROM n;",
        )
        .unwrap();
    }

    /// Every row of both tables, each kept column, in order.
    fn kept_rows(conn: &mut DbConnection) -> Vec<String> {
        KEPT.iter()
            .map(|(table, columns)| {
                text(
                    conn,
                    &format!(
                        "SELECT group_concat(row, ';') AS text FROM (
                            SELECT concat_ws('|', {columns}) AS row FROM {table} ORDER BY id
                        )"
                    ),
                )
            })
            .collect()
    }

    fn columns(conn: &mut DbConnection, table: &str) -> String {
        text(
            conn,
            &format!("SELECT group_concat(name, ', ') AS text FROM pragma_table_info('{table}')"),
        )
    }

    fn text(conn: &mut DbConnection, query: &str) -> String {
        diesel::sql_query(query)
            .get_result::<Text>(conn)
            .unwrap()
            .text
    }
}
