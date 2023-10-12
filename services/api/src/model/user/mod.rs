use std::str::FromStr;

use bencher_json::{Email, JsonSignup, JsonUser, ResourceId, Slug, UserName, UserUuid};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::DbConnection,
    schema::{self, user as user_table},
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
    },
    ApiError,
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
    pub email: String,
    pub admin: bool,
    pub locked: bool,
}

impl QueryUser {
    fn_get!(user, UserId);
    fn_get_id!(user, UserId, UserUuid);
    fn_get_uuid!(user, UserId, UserUuid);

    pub fn get_id_from_email(conn: &mut DbConnection, email: &str) -> Result<UserId, ApiError> {
        schema::user::table
            .filter(schema::user::email.eq(email))
            .select(schema::user::id)
            .first(conn)
            .map_err(ApiError::from)
    }

    pub fn get_email_from_id(conn: &mut DbConnection, id: UserId) -> Result<String, ApiError> {
        schema::user::table
            .filter(schema::user::id.eq(id))
            .select(schema::user::email)
            .first(conn)
            .map_err(ApiError::from)
    }

    pub fn from_resource_id(conn: &mut DbConnection, user: &ResourceId) -> Result<Self, ApiError> {
        schema::user::table
            .filter(resource_id(user)?)
            .first(conn)
            .map_err(ApiError::from)
    }

    pub fn get_admins(conn: &mut DbConnection) -> Result<Vec<QueryUser>, ApiError> {
        schema::user::table
            .filter(schema::user::admin.eq(true))
            .load::<QueryUser>(conn)
            .map_err(ApiError::from)
    }

    pub fn into_json(self) -> Result<JsonUser, ApiError> {
        let Self {
            uuid,
            name,
            slug,
            email,
            admin,
            locked,
            ..
        } = self;
        Ok(JsonUser {
            uuid,
            name,
            slug,
            email: Email::from_str(&email).map_err(ApiError::from)?,
            admin,
            locked,
        })
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = user_table)]
pub struct InsertUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: String,
    pub admin: bool,
    pub locked: bool,
}

impl InsertUser {
    pub fn from_json(conn: &mut DbConnection, signup: JsonSignup) -> Result<Self, ApiError> {
        let JsonSignup {
            name, slug, email, ..
        } = signup;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, user, QueryUser);
        Ok(Self {
            uuid: UserUuid::new(),
            name,
            slug,
            email: email.into(),
            admin: false,
            locked: false,
        })
    }
}
