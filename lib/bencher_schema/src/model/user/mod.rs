use bencher_json::{
    DateTime, Email, JsonPubUser, JsonSignup, JsonUpdateUser, JsonUser, Jwt, Sanitize, UserName,
    UserSlug, UserUuid, organization::member::OrganizationRole,
};
use bencher_token::TokenKey;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;
use url::Url;

use crate::{
    context::{Body, DbConnection, Message, Messenger, NewUserBody},
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    schema::{self, user as user_table},
};

pub mod admin;
pub mod auth;
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

    pub fn get_admins(conn: &mut DbConnection) -> Result<Vec<Self>, HttpError> {
        schema::user::table
            .filter(schema::user::admin.eq(true))
            .load::<QueryUser>(conn)
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

    pub fn accept_invite(
        &self,
        conn: &mut DbConnection,
        token_key: &TokenKey,
        invite: &Jwt,
    ) -> Result<(), HttpError> {
        let insert_org_role = InsertOrganizationRole::from_jwt(conn, token_key, invite, self.id)?;

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
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
            ..
        } = self;
        JsonUser {
            uuid,
            name,
            slug,
            email,
            admin,
            locked,
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
    pub fn new(
        conn: &mut DbConnection,
        name: UserName,
        slug: Option<UserSlug>,
        email: Email,
    ) -> Self {
        let slug = ok_slug!(conn, &name, slug.map(Into::into), user, QueryUser);
        let timestamp = DateTime::now();
        Self {
            uuid: UserUuid::new(),
            name,
            slug: slug.into(),
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
            .select(diesel::dsl::count_distinct(schema::user::id))
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
    pub modified: DateTime,
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
            modified: DateTime::now(),
        }
    }
}
