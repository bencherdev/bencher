use std::{
    str::FromStr,
    string::ToString,
};

use bencher_json::{
    JsonNewProject,
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
use url::Url;
use uuid::Uuid;

use super::user::QueryUser;
use crate::{
    db::schema::{
        self,
        project as project_table,
    },
    diesel::ExpressionMethods,
};

#[derive(Insertable)]
#[table_name = "project_table"]
pub struct InsertProject {
    pub uuid:          String,
    pub owner_id:      i32,
    pub owner_default: bool,
    pub name:          String,
    pub slug:          String,
    pub description:   Option<String>,
    pub url:           Option<String>,
}

impl InsertProject {
    pub fn new(
        conn: &SqliteConnection,
        user_uuid: &Uuid,
        project: JsonNewProject,
    ) -> Result<Self, HttpError> {
        let JsonNewProject {
            name,
            slug,
            description,
            url,
            default,
        } = project;
        let slug = validate_slug(conn, &name, slug);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            owner_id: QueryUser::get_id(conn, user_uuid),
            owner_default: default,
            name,
            slug,
            description,
            url: url.map(|u| u.to_string()),
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

    if schema::project::table
        .filter(schema::project::slug.eq(&slug))
        .first::<QueryProject>(conn)
        .is_ok()
    {
        let rand_suffix = rand::random::<u16>().to_string();
        slug.push_str(&rand_suffix);
        slug
    } else {
        slug
    }
}

// fn validate_email(email: String) -> Result<String, HttpError> {
//     EmailAddress::parse(&email, None)
//         .ok_or(HttpError::for_bad_request(
//             Some(String::from("BadInput")),
//             format!("Failed to parse email: {email}"),
//         ))
//         .map(|email| email.to_string())
// }

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryProject {
    pub id: i32,
    pub uuid: String,
    pub owner_id: i32,
    pub owner_default: bool,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub url: Option<String>,
}

// impl TryInto<JsonUser> for QueryUser {
//     type Error = HttpError;

//     fn try_into(self) -> Result<JsonUser, Self::Error> {
//         let Self {
//             id: _,
//             uuid,
//             name,
//             slug,
//             email,
//         } = self;
//         Ok(JsonUser {
//             uuid: Uuid::from_str(&uuid).map_err(|e| {
//                 HttpError::for_bad_request(
//                     Some(String::from("BadInput")),
//                     format!("Error getting UUID: {e}"),
//                 )
//             })?,
//             name,
//             slug,
//             email,
//         })
//     }
// }

// impl QueryUser {
//     pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> i32 {
//         schema::user::table
//             .filter(schema::user::uuid.eq(&uuid.to_string()))
//             .select(schema::user::id)
//             .first(conn)
//             .unwrap()
//     }
// }
