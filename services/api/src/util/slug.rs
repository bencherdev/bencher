use std::str::FromStr;

use bencher_json::{Slug, MAX_LEN};
use diesel::SqliteConnection;

macro_rules! unwrap_slug {
    ($conn:expr, $name:expr, $slug:expr, $table:ident, $query:ident) => {
        crate::util::slug::validate_slug(
            $conn,
            $name,
            $slug,
            crate::util::slug::slug_exists!($table, $query),
        )
    };
}

pub(crate) use unwrap_slug;

pub type SlugExistsFn = dyn FnOnce(&mut SqliteConnection, &str) -> bool;

pub fn validate_slug(
    conn: &mut SqliteConnection,
    name: &str,
    slug: Option<Slug>,
    exists: Box<SlugExistsFn>,
) -> String {
    let slug = if let Some(slug) = slug {
        slug.into()
    } else {
        slug::slugify(name)
    };

    let slug = if exists(conn, &slug) {
        let rand_suffix = rand::random::<u32>().to_string();
        format!("{slug}-{rand_suffix}")
    } else {
        slug
    };

    let slug = if slug.len() > MAX_LEN {
        slug::slugify(slug.split_at(MAX_LEN).0)
    } else {
        slug
    };

    Slug::from_str(&slug).expect("Invalid slug").into()
}

macro_rules! slug_exists {
    ($table:ident, $query:ident) => {
        Box::new(|conn, slug| {
            schema::$table::table
                .filter(schema::$table::slug.eq(slug))
                .first::<$query>(conn)
                .is_ok()
        })
    };
}

pub(crate) use slug_exists;
