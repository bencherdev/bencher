use bencher_json::Slug;
use dropshot::HttpError;

use crate::{context::DbConnection, model::project::ProjectId};

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
    let new_slug = Slug::unwrap_or_new(name, slug);
    Ok(if exists(conn, project_id, new_slug.as_ref()) {
        new_slug.with_rand_suffix()
    } else {
        new_slug
    })
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
