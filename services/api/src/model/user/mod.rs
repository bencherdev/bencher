use bencher_json::{Email, JsonSignup, JsonUser, ResourceId, Slug, UserName, UserUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema::{self, user as user_table},
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
    },
};

pub mod auth;
pub mod token;

crate::util::typed_id::typed_id!(UserId);

fn_resource_id!(user);

#[derive(diesel::Queryable)]
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
    fn_get!(user, UserId);
    fn_get_id!(user, UserId, UserUuid);
    fn_get_uuid!(user, UserId, UserUuid);

    pub fn get_id_from_email(conn: &mut DbConnection, email: &str) -> Result<UserId, HttpError> {
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

    pub fn from_resource_id(conn: &mut DbConnection, user: &ResourceId) -> Result<Self, HttpError> {
        schema::user::table
            .filter(resource_id(user)?)
            .first(conn)
            .map_err(resource_not_found_err!(User, user.clone()))
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

#[derive(diesel::Insertable)]
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
    pub fn from_json(conn: &mut DbConnection, signup: JsonSignup) -> Self {
        let JsonSignup {
            name, slug, email, ..
        } = signup;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, user, QueryUser);
        Self {
            uuid: UserUuid::new(),
            name,
            slug,
            email,
            admin: false,
            locked: false,
        }
    }
}
