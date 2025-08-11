use bencher_json::Slug;

use crate::{context::DbConnection, model::project::ProjectId};

pub type SlugExistsFn = dyn FnOnce(&mut DbConnection, Option<ProjectId>, &str) -> bool;

pub fn validate_slug<N, S>(
    conn: &mut DbConnection,
    project_id: Option<ProjectId>,
    name: N,
    slug: Option<S>,
    exists: Box<SlugExistsFn>,
) -> S
where
    N: AsRef<str> + std::fmt::Display,
    S: Into<Slug> + From<Slug>,
{
    let slug = slug.map(Into::into);
    let new_slug = Slug::unwrap_or_new(name, slug);
    if exists(conn, project_id, new_slug.as_ref()) {
        new_slug.with_rand_suffix()
    } else {
        new_slug
    }
    .into()
}

macro_rules! ok_slug {
    ($conn:expr, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        $crate::macros::slug::validate_slug(
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
    ($conn:expr, $project_id:expr, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        $crate::macros::slug::validate_slug(
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
