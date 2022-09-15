use std::str::FromStr;

use bencher_json::{jwt::JsonWebToken, JsonSignup, JsonUser, ResourceId};
use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use dropshot::{HttpError, RequestContext};
use email_address_parser::EmailAddress;
use uuid::Uuid;

use crate::{
    diesel::ExpressionMethods,
    schema::{self, user as user_table},
    util::{http_error, map_http_error, resource_id::fn_resource_id, slug::unwrap_slug, Context},
};

pub mod auth;
pub mod organization;
pub mod project;
pub mod token;

#[derive(Insertable)]
#[diesel(table_name = user_table)]
pub struct InsertUser {
    pub uuid: String,
    pub name: String,
    pub slug: String,
    pub email: String,
    pub admin: bool,
    pub locked: bool,
}

impl InsertUser {
    pub fn from_json(conn: &mut SqliteConnection, signup: JsonSignup) -> Result<Self, HttpError> {
        let JsonSignup {
            name,
            slug,
            email,
            invite: _,
        } = signup;
        validate_email(&email)?;
        let slug = unwrap_slug!(conn, &name, slug, user, QueryUser);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name,
            slug,
            email,
            admin: false,
            locked: false,
        })
    }
}

fn validate_email(email: &str) -> Result<EmailAddress, HttpError> {
    EmailAddress::parse(email, None).ok_or_else(|| http_error!("Failed to get user."))
}

fn_resource_id!(user);

#[derive(Queryable)]
pub struct QueryUser {
    pub id: i32,
    pub uuid: String,
    pub name: String,
    pub slug: String,
    pub email: String,
    pub admin: bool,
    pub locked: bool,
}

impl QueryUser {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::user::table
            .filter(schema::user::uuid.eq(uuid.to_string()))
            .select(schema::user::id)
            .first(conn)
            .map_err(map_http_error!("Failed to get user."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::user::table
            .filter(schema::user::id.eq(id))
            .select(schema::user::uuid)
            .first(conn)
            .map_err(map_http_error!("Failed to get user."))?;
        Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get user."))
    }

    pub fn get_id_from_email(conn: &mut SqliteConnection, email: &str) -> Result<i32, HttpError> {
        schema::user::table
            .filter(schema::user::email.eq(email))
            .select(schema::user::id)
            .first(conn)
            .map_err(map_http_error!("Failed to get user."))
    }

    pub fn get_email_from_id(conn: &mut SqliteConnection, id: i32) -> Result<String, HttpError> {
        schema::user::table
            .filter(schema::user::id.eq(id))
            .select(schema::user::email)
            .first(conn)
            .map_err(map_http_error!("Failed to get user."))
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        user: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::user::table
            .filter(resource_id(user)?)
            .first(conn)
            .map_err(map_http_error!("Failed to get user."))
    }

    pub fn into_json(self) -> Result<JsonUser, HttpError> {
        let Self {
            id: _,
            uuid,
            name,
            slug,
            email,
            admin,
            locked,
        } = self;
        Ok(JsonUser {
            uuid: Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get user."))?,
            name,
            slug,
            email,
            admin,
            locked,
        })
    }
}
