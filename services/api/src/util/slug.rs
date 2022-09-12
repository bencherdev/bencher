use diesel::SqliteConnection;

pub fn validate_slug(
    conn: &mut SqliteConnection,
    name: &str,
    slug: Option<String>,
    exists: Box<dyn FnOnce(&mut SqliteConnection, &str) -> bool>,
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
