#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Search(String);

#[cfg(feature = "db")]
mod search_query {
    use super::Search;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for Search
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            // https://docs.rs/diesel/latest/diesel/serialize/trait.ToSql.html#examples
            out.set_value(format!("%{}%", self.0));
            Ok(diesel::serialize::IsNull::No)
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::Search;

    #[test]
    fn search_serde_round_trip() {
        // The `%...%` wrapping only happens in the SQL serialization layer,
        // not in the serde representation.
        let search: Search = serde_json::from_str("\"my query\"").expect("deserialize search");
        assert_eq!(
            "\"my query\"",
            serde_json::to_string(&search).expect("serialize search")
        );
    }
}

#[cfg(test)]
#[cfg(feature = "db")]
mod db_tests {
    use diesel::{
        Connection as _, IntoSql as _, RunQueryDsl as _, SqliteConnection,
        TextExpressionMethods as _,
    };
    use pretty_assertions::assert_eq;

    use super::Search;

    /// `(query, bound SQL value)` pairs documenting the current `ToSql` behavior:
    /// the query is wrapped in `%...%` and SQL `LIKE` special characters
    /// (`%`, `_`, `'`, `\`) are passed through unescaped.
    const LIKE_CASES: [(&str, &str); 8] = [
        ("foo", "%foo%"),
        ("my query", "%my query%"),
        ("", "%%"),
        ("50%", "%50%%"),
        ("a_b", "%a_b%"),
        ("O'Brien", "%O'Brien%"),
        ("a\\b", "%a\\b%"),
        ("%", "%%%"),
    ];

    fn connection() -> SqliteConnection {
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database")
    }

    #[test]
    fn search_to_sql_wraps_in_percent() {
        let mut conn = connection();
        for (query, expected) in LIKE_CASES {
            let search = Search(query.to_owned());
            let value: String = diesel::select(search.into_sql::<diesel::sql_types::Text>())
                .get_result(&mut conn)
                .expect("Failed to select Search as String");
            assert_eq!(expected, value, "{query}");
        }
    }

    #[test]
    fn search_like_special_chars_act_as_wildcards() {
        // `(haystack, query, matches)` documenting that `%` and `_` in the user's
        // query act as `LIKE` wildcards because they are not escaped.
        let cases = [
            ("price 50x off", "50%", true),
            ("price 50% off", "50%", true),
            ("abc", "a_c", true),
            ("a_c", "a_c", true),
            ("abc", "xyz", false),
        ];
        let mut conn = connection();
        for (haystack, query, expected) in cases {
            let search = Search(query.to_owned());
            let matched: bool = diesel::select(
                haystack
                    .into_sql::<diesel::sql_types::Text>()
                    .like(search.into_sql::<diesel::sql_types::Text>()),
            )
            .get_result(&mut conn)
            .expect("Failed to select LIKE result");
            assert_eq!(expected, matched, "`{haystack}` LIKE `%{query}%`");
        }
    }
}
