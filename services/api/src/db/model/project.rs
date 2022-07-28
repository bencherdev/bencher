use std::{
    str::FromStr,
    string::ToString,
};

use bencher_json::{
    JsonNewProject,
    JsonProject,
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
    util::http_error,
};

const PROJECT_ERROR: &str = "Failed to get project.";
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
    pub fn from_json(
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
            owner_id: QueryUser::get_id(conn, user_uuid)?,
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

impl QueryProject {
    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonProject, HttpError> {
        let Self {
            id: _,
            uuid,
            owner_id,
            owner_default,
            name,
            slug,
            description,
            url,
        } = self;
        Ok(JsonProject {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(PROJECT_ERROR))?,
            owner_uuid: QueryUser::get_uuid(conn, owner_id)?,
            owner_default,
            name,
            slug,
            description,
            url: ok_url(url.as_deref())?,
        })
    }
}

fn ok_url(url: Option<&str>) -> Result<Option<Url>, HttpError> {
    Ok(if let Some(url) = url {
        Some(Url::parse(url).map_err(|_| http_error!(PROJECT_ERROR))?)
    } else {
        None
    })
}
