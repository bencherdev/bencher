use std::str::FromStr;

use bencher_json::JsonUser;
use diesel::{
    Insertable,
    Queryable,
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
    db::{
        schema,
        schema::user as user_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
};

#[derive(Insertable)]
#[table_name = "user_table"]
pub struct InsertUser {
    pub uuid:  String,
    pub name:  String,
    pub slug:  String,
    pub email: String,
}

impl InsertUser {
    pub fn new(user: JsonUser) -> Result<Self, HttpError> {
        let JsonUser { name, slug, email } = user;
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            name,
            slug: validate_slug(slug)?,
            email: validate_email(email)?,
        })
    }
}

fn validate_slug(slug: String) -> Result<String, HttpError> {
    let true_slug = slug::slugify(&slug);
    if slug != true_slug {
        return Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Slug was not valid: {slug}"),
        ));
    }
    Ok(slug)
}

fn validate_email(email: String) -> Result<String, HttpError> {
    EmailAddress::parse(&email, None)
        .ok_or(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to parse email: {email}"),
        ))
        .map(|email| email.to_string())
}
