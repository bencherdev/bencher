use std::str::FromStr;

use bencher_json::Slug;

use crate::{context::DbConnection, error::issue_error, model::project::ProjectId};

macro_rules! ok_slug {
    ($conn:expr, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        crate::util::slug::validate_slug(
            $conn,
            None,
            $name,
            $slug,
            crate::util::slug::slug_exists!($table, $query),
        )
    };
}

use dropshot::HttpError;
use http::StatusCode;
pub(crate) use ok_slug;

macro_rules! ok_child_slug {
    ($conn:expr, $parent:ident, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        crate::util::slug::validate_slug(
            $conn,
            Some($parent),
            $name,
            $slug,
            crate::util::slug::child_slug_exists!($table, $query),
        )
    };
}

pub(crate) use ok_child_slug;

pub type SlugExistsFn = dyn FnOnce(&mut DbConnection, Option<ProjectId>, &str) -> bool;

pub fn validate_slug<S>(
    conn: &mut DbConnection,
    parent: Option<ProjectId>,
    name: S,
    slug: Option<Slug>,
    exists: Box<SlugExistsFn>,
) -> Result<Slug, HttpError>
where
    S: AsRef<str> + std::fmt::Display,
{
    let mut slug = if let Some(slug) = slug {
        slug.into()
    } else {
        slug::slugify(&name)
    };

    if slug.len() > Slug::MAX {
        slug = slug::slugify(slug.split_at(Slug::MAX).0);
    }

    if exists(conn, parent, &slug) {
        Ok(Slug::new(&slug))
    } else {
        Slug::from_str(&slug).map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid slug",
                &format!("An invalid slug was generated ({slug}) from the name: {name}"),
                e,
            )
        })
    }
}

macro_rules! slug_exists {
    ($table:ident, $query:ident) => {
        Box::new(|conn, _parent, slug| {
            schema::$table::table
                .filter(schema::$table::slug.eq(slug))
                .first::<$query>(conn)
                .is_ok()
        })
    };
}

pub(crate) use slug_exists;

macro_rules! child_slug_exists {
    ($table:ident, $query:ident) => {
        Box::new(|conn, parent, slug| {
            schema::$table::table
                .filter(schema::$table::project_id.eq(parent.expect("Missing Project ID")))
                .filter(schema::$table::slug.eq(slug))
                .first::<$query>(conn)
                .is_ok()
        })
    };
}

pub(crate) use child_slug_exists;
