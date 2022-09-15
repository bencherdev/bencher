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
    slug: Option<String>,
    exists: Box<SlugExistsFn>,
) -> String {
    let mut slug = slug
        .map(|s| {
            if s == slug::slugify(&s) {
                s
            } else {
                slug::slugify(name)
            }
        })
        .unwrap_or_else(|| slug::slugify(name));

    if exists(conn, &slug) {
        let rand_suffix = rand::random::<u32>().to_string();
        slug.push_str(&rand_suffix);
        slug
    } else {
        slug
    }
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
