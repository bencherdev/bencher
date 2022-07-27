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

// impl InsertUser {
//     pub fn new(conn: &SqliteConnection, signup: JsonSignup) -> Result<Self,
// HttpError> {         let JsonSignup { name, slug, email } = signup;
//         let slug = validate_slug(conn, &name, slug)?;
//         Ok(Self {
//             uuid: Uuid::new_v4().to_string(),
//             name,
//             slug,
//             email: validate_email(email)?,
//         })
//     }
// }

// fn validate_slug(
//     conn: &SqliteConnection,
//     name: &str,
//     slug: Option<String>,
// ) -> Result<String, HttpError> {
//     let mut slug = if let Some(slug) = slug {
//         let true_slug = slug::slugify(&slug);
//         if slug != true_slug {
//             return Err(HttpError::for_bad_request(
//                 Some(String::from("BadInput")),
//                 format!("Slug was not valid: {slug}"),
//             ));
//         }
//         slug
//     } else {
//         slug::slugify(name)
//     };

//     if schema::user::table
//         .filter(schema::user::slug.eq(&slug))
//         .first::<QueryUser>(conn)
//         .is_ok()
//     {
//         let rand_suffix = rand::random::<u16>().to_string();
//         slug.push_str(&rand_suffix);
//         Ok(slug)
//     } else {
//         Ok(slug)
//     }
// }

// fn validate_email(email: String) -> Result<String, HttpError> {
//     EmailAddress::parse(&email, None)
//         .ok_or(HttpError::for_bad_request(
//             Some(String::from("BadInput")),
//             format!("Failed to parse email: {email}"),
//         ))
//         .map(|email| email.to_string())
// }

// #[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
// pub struct QueryUser {
//     pub id:    i32,
//     pub uuid:  String,
//     pub name:  String,
//     pub slug:  String,
//     pub email: String,
// }

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
