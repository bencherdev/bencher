use std::str::FromStr;

use bencher_json::Slug;
use dropshot::HttpError;
use http::StatusCode;

use crate::{context::DbConnection, error::issue_error, model::project::ProjectId};

pub type SlugExistsFn = dyn FnOnce(&mut DbConnection, Option<ProjectId>, &str) -> bool;

pub fn validate_slug<S>(
    conn: &mut DbConnection,
    project_id: Option<ProjectId>,
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

    if exists(conn, project_id, &slug) {
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

macro_rules! ok_slug {
    ($conn:expr, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        crate::util::slug::validate_slug(
            $conn,
            None,
            $name,
            $slug,
            Box::new(|conn, _project_id, slug| {
                schema::$table::table
                    .filter(schema::$table::slug.eq(slug))
                    .first::<$query>(conn)
                    .is_ok()
            }),
        )
    };
    ($conn:expr, $project_id:ident, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        crate::util::slug::validate_slug(
            $conn,
            Some($project_id),
            $name,
            $slug,
            Box::new(|conn, project_id, slug| {
                schema::$table::table
                    .filter(schema::$table::project_id.eq(project_id.expect("Missing Project ID")))
                    .filter(schema::$table::slug.eq(slug))
                    .first::<$query>(conn)
                    .is_ok()
            }),
        )
    };
}

pub(crate) use ok_slug;
