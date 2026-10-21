use bencher_json::{
    DateTime, Email, JsonPubUser, JsonSignup, JsonUpdateUser, JsonUser, Jwt, Sanitize, UserName,
    UserSlug, UserUuid, organization::member::OrganizationRole,
};
use bencher_token::TokenKey;
use diesel::{
    AggregateExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;
use slog::Logger;
use url::Url;

use crate::{
    ApiContext, auth_conn,
    context::{Body, DbConnection, Message, Messenger, NewUserBody},
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
        sql::max,
    },
    schema::{self, user as user_table},
    write_conn,
};

pub mod actor;
pub mod admin;
pub mod auth;
pub mod key;
#[cfg(feature = "plus")]
pub mod oauth;
#[cfg(feature = "plus")]
pub mod oauth_authorization;
pub mod public;
pub mod token;

crate::macros::typed_id::typed_id!(UserId);

#[macro_export]
macro_rules! same_user {
    ($auth_user:ident, $rbac:expr, $user_uuid:expr) => {
        if !($auth_user.is_admin(&$rbac) || $auth_user.uuid() == $user_uuid) {
            return Err($crate::error::forbidden_error(format!("User is not admin and the authenticated user ({auth_user}) does not match the requested user ({requested_user})", auth_user = $auth_user.uuid(), requested_user = $user_uuid)));
        }
    };
}

pub use same_user;

use super::organization::{
    InsertOrganization, QueryOrganization, organization_role::InsertOrganizationRole,
};

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryUser {
    pub id: UserId,
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: UserSlug,
    pub email: Email,
    /// When the user last proved they hold `email`, cleared when it changes.
    pub email_verified: Option<DateTime>,
    pub admin: bool,
    pub locked: bool,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryUser {
    fn_eq_resource_id!(user, UserResourceId);
    fn_from_resource_id!(user, User, UserResourceId);

    fn from_uuid(conn: &mut DbConnection, uuid: UserUuid) -> Result<Self, HttpError> {
        schema::user::table
            .filter(schema::user::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(User, uuid))
    }

    fn_get!(user, UserId);
    fn_get_id!(user, UserId, UserUuid);
    fn_get_uuid!(user, UserId, UserUuid);

    pub fn id(&self) -> UserId {
        self.id
    }

    pub fn uuid(&self) -> UserUuid {
        self.uuid
    }

    pub fn get_id_from_email(conn: &mut DbConnection, email: &Email) -> Result<UserId, HttpError> {
        schema::user::table
            .filter(schema::user::email.eq(email))
            .select(schema::user::id)
            .first(conn)
            .map_err(resource_not_found_err!(User, email))
    }

    pub fn get_email_from_id(conn: &mut DbConnection, id: UserId) -> Result<String, HttpError> {
        schema::user::table
            .filter(schema::user::id.eq(id))
            .select(schema::user::email)
            .first(conn)
            .map_err(resource_not_found_err!(User, id))
    }

    pub fn get_with_email(conn: &mut DbConnection, email: &Email) -> Result<Self, HttpError> {
        schema::user::table
            .filter(schema::user::email.eq(email))
            .first(conn)
            .map_err(resource_not_found_err!(User, email))
    }

    /// Record that the user proved they hold `email` at `now`, unless their email is no longer it.
    pub fn verify_email(
        conn: &mut DbConnection,
        id: UserId,
        email: &Email,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::user::table
                .filter(schema::user::id.eq(id))
                .filter(schema::user::email.eq(email)),
        )
        .set(schema::user::email_verified.eq(now))
        .execute(conn)
    }

    pub fn get_admins(conn: &mut DbConnection) -> Result<Vec<Self>, HttpError> {
        schema::user::table
            .filter(schema::user::admin.eq(true))
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(User, true))
    }

    /// Check to see if the user account has been locked
    pub fn check_is_locked(&self) -> Result<(), HttpError> {
        if self.locked {
            let mut query_user = self.clone();
            query_user.sanitize();
            Err(forbidden_error(format!(
                "Your account has been locked. Please contact support: {query_user:?}",
            )))
        } else {
            Ok(())
        }
    }

    /// Rate limit authentication attempts for this user
    #[cfg(feature = "plus")]
    pub fn rate_limit_auth(&self, context: &ApiContext) -> Result<(), HttpError> {
        context.rate_limiting.auth_attempt(self.uuid)
    }

    pub async fn accept_invite(
        &self,
        context: &ApiContext,
        token_key: &TokenKey,
        invite: &Jwt,
    ) -> Result<(), HttpError> {
        let insert_org_role =
            InsertOrganizationRole::from_jwt(auth_conn!(context), token_key, invite, self.id)?;

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(OrganizationRole, insert_org_role))?;

        Ok(())
    }

    pub fn into_json(self) -> JsonUser {
        let Self {
            uuid,
            name,
            slug,
            email,
            admin,
            locked,
            created,
            ..
        } = self;
        JsonUser {
            uuid,
            name,
            slug,
            email,
            admin,
            locked,
            created: Some(created),
        }
    }

    pub fn into_pub_json(self) -> JsonPubUser {
        let Self {
            uuid, name, slug, ..
        } = self;
        JsonPubUser { uuid, name, slug }
    }
}

impl Sanitize for QueryUser {
    fn sanitize(&mut self) {
        self.email.sanitize();
    }
}

#[derive(Debug, Clone, diesel::Insertable)]
#[diesel(table_name = user_table)]
pub struct InsertUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: UserSlug,
    pub email: Email,
    pub admin: bool,
    pub locked: bool,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertUser {
    fn new(conn: &mut DbConnection, name: UserName, slug: Option<UserSlug>, email: Email) -> Self {
        let slug = ok_slug!(conn, &name, slug, user, QueryUser);
        let timestamp = DateTime::now();
        Self {
            uuid: UserUuid::new(),
            name,
            slug,
            email,
            admin: false,
            locked: false,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_json(
        conn: &mut DbConnection,
        token_key: &TokenKey,
        json_signup: &JsonSignup,
    ) -> Result<Self, HttpError> {
        let JsonSignup {
            name, slug, email, ..
        } = json_signup.clone();
        let mut insert_user = Self::new(conn, name, slug, email);

        let count = schema::user::table
            .select(diesel::dsl::count(schema::user::id).aggregate_distinct())
            .first::<i64>(conn)
            .map_err(resource_not_found_err!(User, json_signup))?;
        // The first user to signup is admin
        if count == 0 {
            insert_user.admin = true;
        }

        // Insert user
        diesel::insert_into(schema::user::table)
            .values(&insert_user)
            .execute(conn)
            .map_err(resource_conflict_err!(User, insert_user))?;
        let query_user = QueryUser::from_uuid(conn, insert_user.uuid)?;

        let insert_org_role = if let Some(invite) = &json_signup.invite {
            InsertOrganizationRole::from_jwt(conn, token_key, invite, query_user.id)?
        } else if let Some(organization_uuid) = json_signup.claim {
            let organization_id = QueryOrganization::get_id(conn, organization_uuid)?;
            let timestamp = DateTime::now();
            InsertOrganizationRole {
                user_id: query_user.id,
                organization_id,
                role: OrganizationRole::Leader,
                created: timestamp,
                modified: timestamp,
            }
        } else {
            // Create an organization for the user
            let insert_organization = InsertOrganization::from_user(conn, &query_user);
            diesel::insert_into(schema::organization::table)
                .values(&insert_organization)
                .execute(conn)
                .map_err(resource_conflict_err!(Organization, insert_organization))?;
            let organization_id = QueryOrganization::get_id(conn, insert_organization.uuid)?;

            let timestamp = DateTime::now();
            // Connect the user to the organization as a `Leader`
            InsertOrganizationRole {
                user_id: query_user.id,
                organization_id,
                role: OrganizationRole::Leader,
                created: timestamp,
                modified: timestamp,
            }
        };

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
            .map_err(resource_conflict_err!(OrganizationRole, insert_org_role))?;

        Ok(insert_user)
    }

    /// Rate limit authentication attempts for this user
    #[cfg(feature = "plus")]
    pub fn rate_limit_auth(&self, context: &ApiContext) -> Result<(), HttpError> {
        context.rate_limiting.auth_attempt(self.uuid)
    }

    pub fn notify(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        messenger: &Messenger,
        console_url: &Url,
        invited: bool,
        method: &str,
    ) -> Result<(), HttpError> {
        if !self.admin {
            let admins = QueryUser::get_admins(conn)?;
            for admin in admins {
                let message = Message {
                    to_name: Some(admin.name.clone().into()),
                    to_email: admin.email.into(),
                    subject: Some("🐰 New Bencher User".into()),
                    body: Some(Body::NewUser(NewUserBody {
                        admin: admin.name.clone().into(),
                        console_url: console_url.clone(),
                        name: self.name.clone().into(),
                        email: self.email.clone().into(),
                        invited,
                        method: method.to_owned(),
                    })),
                };
                messenger.send(log, message);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = user_table)]
pub struct UpdateUser {
    pub name: Option<UserName>,
    pub slug: Option<UserSlug>,
    pub email: Option<Email>,
    pub admin: Option<bool>,
    pub locked: Option<bool>,
}

impl From<JsonUpdateUser> for UpdateUser {
    fn from(update: JsonUpdateUser) -> Self {
        let JsonUpdateUser {
            name,
            slug,
            email,
            admin,
            locked,
        } = update;
        Self {
            name,
            slug,
            email,
            admin,
            locked,
        }
    }
}

/// Set `modified` for a write to a user at `now`, strictly after the last write even within
/// one second, so an email change link bound to it ends with any later update to the user.
#[diesel::dsl::auto_type(no_type_alias)]
pub fn bump_modified(now: DateTime) -> _ {
    user_table::modified.eq(max(now, user_table::modified + 1i64))
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Email};
    use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
    use diesel_migrations::MigrationHarness as _;
    use pretty_assertions::assert_eq;

    use super::QueryUser;
    use crate::{run_migrations, test_util::setup_test_db};

    const EMAIL_VERIFIED_MIGRATION: &str = "20261014120000";
    /// Every user column, in order, before the migration.
    const USER_COLUMNS: &str = "id, uuid, name, slug, email, admin, locked, created, modified";
    /// The tables whose rows hang off a user, which rebuilding the user table must leave alone.
    const CHILD_TABLES: [&str; 5] = [
        "organization_role",
        "user_key",
        "token",
        "oauth_grant",
        "oauth_token",
    ];

    #[derive(diesel::QueryableByName)]
    struct Text {
        #[diesel(sql_type = diesel::sql_types::Text)]
        text: String,
    }

    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    // Rebuilding the user table must keep every user and everything that hangs off one,
    // with the new column right after the email.
    #[test]
    fn email_verified_migration_keeps_every_user() {
        let mut conn = setup_test_db();
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == EMAIL_VERIFIED_MIGRATION {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        seed_users(&mut conn, 1024);
        let users = user_rows(&mut conn);
        let children = child_counts(&mut conn);

        // The path the server takes at startup.
        run_migrations(&mut conn).unwrap();
        assert_eq!(
            text(
                &mut conn,
                "SELECT group_concat(name, ', ') AS text FROM pragma_table_info('user')"
            ),
            "id, uuid, name, slug, email, email_verified, admin, locked, created, modified"
        );
        assert_eq!(user_rows(&mut conn), users);
        assert_eq!(child_counts(&mut conn), children);
        assert_eq!(
            count(
                &mut conn,
                "SELECT COUNT(*) AS count FROM user WHERE email_verified IS NOT NULL"
            ),
            0,
            "no address is asserted verified before its user proves it"
        );
        assert_eq!(
            count(
                &mut conn,
                "SELECT COUNT(*) AS count FROM pragma_foreign_key_check"
            ),
            0
        );

        conn.batch_execute(
            "UPDATE user SET email_verified = 1 WHERE id % 2 = 0; PRAGMA foreign_keys = OFF",
        )
        .unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == EMAIL_VERIFIED_MIGRATION {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        assert_eq!(user_rows(&mut conn), users);
        assert_eq!(child_counts(&mut conn), children);
    }

    // The update is guarded by the address, so a proof of an old address never verifies a new one.
    #[test]
    fn verify_email_needs_the_current_address() {
        let mut conn = setup_test_db();
        seed_users(&mut conn, 1);
        let email: Email = "user1@example.com".parse().unwrap();
        let id = QueryUser::get_id_from_email(&mut conn, &email).unwrap();

        let old_email: Email = "old@example.com".parse().unwrap();
        assert_eq!(
            QueryUser::verify_email(&mut conn, id, &old_email, DateTime::TEST).unwrap(),
            0
        );
        assert_eq!(QueryUser::get(&mut conn, id).unwrap().email_verified, None);

        assert_eq!(
            QueryUser::verify_email(&mut conn, id, &email, DateTime::TEST).unwrap(),
            1
        );
        assert_eq!(
            QueryUser::get(&mut conn, id).unwrap().email_verified,
            Some(DateTime::TEST)
        );
    }

    /// `n` users, each with an organization role, a user key, an API token, and an OAuth grant
    /// with a token, written with only the columns the user table had before the migration.
    fn seed_users(conn: &mut diesel::SqliteConnection, n: u32) {
        conn.batch_execute(&format!(
            "INSERT INTO organization (id, uuid, name, slug, created, modified)
                VALUES (1, '00000000-0000-0000-0000-000000000001', 'Organization', 'organization', 0, 0);
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {n})
            INSERT INTO user ({USER_COLUMNS})
                SELECT i, printf('00000000-0000-4000-8000-%012d', i), 'User ' || i, 'user-' || i,
                    'user' || i || '@example.com', i % 7 = 0, i % 5 = 0, 1700000000 + i, 1700000000 + 2 * i
                FROM n;
            INSERT INTO organization_role (user_id, organization_id, role, created, modified)
                SELECT id, 1, 'leader', created, modified FROM user;
            INSERT INTO user_key (uuid, user_id, name, key_hash, creation, expiration)
                SELECT printf('00000000-0000-4000-9000-%012d', id), id, 'Key', 'hash-' || id, created, created + 1
                FROM user;
            INSERT INTO token (uuid, user_id, name, jwt, creation, expiration)
                SELECT printf('00000000-0000-4000-a000-%012d', id), id, 'Token', 'jwt-' || id, created, created + 1
                FROM user;
            INSERT INTO oauth_grant (uuid, user_id, user_key_id, client_id, client_name, scope, resource, created, modified, expiration)
                SELECT printf('00000000-0000-4000-b000-%012d', user_id), user_id, id,
                    'https://client.example/client.json', 'Client', 'projects:read', 'https://api.example.com/mcp',
                    creation, creation, expiration
                FROM user_key;
            INSERT INTO oauth_token (grant_id, kind, token_hash, created, expiration)
                SELECT id, 0, 'token-' || id, created, expiration FROM oauth_grant;"
        ))
        .unwrap();
    }

    /// Every user, every old column, in order.
    fn user_rows(conn: &mut diesel::SqliteConnection) -> String {
        text(
            conn,
            &format!(
                "SELECT group_concat(row, ';') AS text FROM (
                    SELECT concat_ws('|', {USER_COLUMNS}) AS row FROM user ORDER BY id
                )"
            ),
        )
    }

    fn child_counts(conn: &mut diesel::SqliteConnection) -> Vec<i64> {
        CHILD_TABLES
            .iter()
            .map(|table| count(conn, &format!("SELECT COUNT(*) AS count FROM {table}")))
            .collect()
    }

    fn text(conn: &mut diesel::SqliteConnection, query: &str) -> String {
        diesel::sql_query(query)
            .get_result::<Text>(conn)
            .unwrap()
            .text
    }

    fn count(conn: &mut diesel::SqliteConnection, query: &str) -> i64 {
        diesel::sql_query(query)
            .get_result::<Count>(conn)
            .unwrap()
            .count
    }
}
