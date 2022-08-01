use std::str::FromStr;

use bencher_json::{
    JsonSignup,
    JsonUser,
};
use diesel::{
    Insertable,
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use email_address_parser::EmailAddress;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::{
    db::schema::{
        self,
        user as user_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const USER_ERROR: &str = "Failed to get user";

#[derive(Insertable)]
#[table_name = "user_table"]
pub struct InsertUser {
    pub uuid:  String,
    pub name:  String,
    pub slug:  String,
    pub email: String,
}

impl InsertUser {
    pub fn new(conn: &SqliteConnection, signup: JsonSignup) -> Result<Self, HttpError> {
        let JsonSignup { name, slug, email } = signup;
        let slug = validate_slug(conn, &name, slug);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name,
            slug,
            email: validate_email(email)?,
        })
    }
}

fn validate_slug(conn: &SqliteConnection, name: &str, slug: Option<String>) -> String {
    let mut slug = slug
        .map(|s| {
            if s == slug::slugify(&s) {
                s
            } else {
                slug::slugify(name)
            }
        })
        .unwrap_or_else(|| slug::slugify(name));

    if schema::user::table
        .filter(schema::user::slug.eq(&slug))
        .first::<QueryUser>(conn)
        .is_ok()
    {
        let rand_suffix = rand::random::<u32>().to_string();
        slug.push_str(&rand_suffix);
        slug
    } else {
        slug
    }
}

fn validate_email(email: String) -> Result<String, HttpError> {
    EmailAddress::parse(&email, None)
        .ok_or(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to parse email: {email}"),
        ))
        .map(|email| email.to_string())
}

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryUser {
    pub id:    i32,
    pub uuid:  String,
    pub name:  String,
    pub slug:  String,
    pub email: String,
}

impl TryInto<JsonUser> for QueryUser {
    type Error = HttpError;

    fn try_into(self) -> Result<JsonUser, Self::Error> {
        let Self {
            id: _,
            uuid,
            name,
            slug,
            email,
        } = self;
        Ok(JsonUser {
            uuid: Uuid::from_str(&uuid).map_err(|e| {
                HttpError::for_bad_request(
                    Some(String::from("BadInput")),
                    format!("Error getting UUID: {e}"),
                )
            })?,
            name,
            slug,
            email,
        })
    }
}

impl QueryUser {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::user::table
            .filter(schema::user::uuid.eq(&uuid.to_string()))
            .select(schema::user::id)
            .first(conn)
            .map_err(|_| http_error!(USER_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::user::table
            .filter(schema::user::id.eq(id))
            .select(schema::user::uuid)
            .first(conn)
            .map_err(|_| http_error!(USER_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(USER_ERROR))
    }
}
