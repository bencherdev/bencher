use bencher_json::{Email, JsonSignup, JsonUser, ResourceId, Slug, UserName, UserUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema::{self, user as user_table},
    util::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_from_resource_id, fn_resource_id},
        slug::ok_slug,
    },
};

pub mod admin;
pub mod auth;
pub mod token;

crate::util::typed_id::typed_id!(UserId);

macro_rules! same_user {
    ($auth_user:ident, $rbac:expr, $user_id:expr) => {
        if !($auth_user.is_admin(&$rbac) || $auth_user.id == $user_id) {
            return Err(crate::error::forbidden_error(format!("User is not admin and the authenticated user ({auth_user}) does not match the requested user ({requested_user})", auth_user = $auth_user.id, requested_user = $user_id)));
        }
    };
}

pub(crate) use same_user;

#[derive(Debug, diesel::Queryable)]
pub struct QueryUser {
    pub id: UserId,
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub admin: bool,
    pub locked: bool,
}

impl QueryUser {
    fn_resource_id!(user);
    fn_from_resource_id!(user, User, true);

    fn_get!(user, UserId);
    fn_get_id!(user, UserId, UserUuid);
    fn_get_uuid!(user, UserId, UserUuid);

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

    pub fn get_admins(conn: &mut DbConnection) -> Result<Vec<QueryUser>, HttpError> {
        schema::user::table
            .filter(schema::user::admin.eq(true))
            .load::<QueryUser>(conn)
            .map_err(resource_not_found_err!(User, true))
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
}

#[derive(Debug, Clone, diesel::Insertable)]
#[diesel(table_name = user_table)]
pub struct InsertUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub admin: bool,
    pub locked: bool,
}

impl InsertUser {
    pub fn from_json(conn: &mut DbConnection, signup: JsonSignup) -> Result<Self, HttpError> {
        let JsonSignup {
            name, slug, email, ..
        } = signup;
        let slug = ok_slug!(conn, &name, slug, user, QueryUser)?;
        Ok(Self {
            uuid: UserUuid::new(),
            name,
            slug,
            email,
            admin: false,
            locked: false,
        })
    }
}
