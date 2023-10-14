macro_rules! into_json {
    ($context:expr, $($field:tt)*) => {
        |query| {
            crate::util::error::database_map($context, query.into_json($($field)*))
        }
    };
}

pub(crate) use into_json;

pub fn database_map<C, T, E>(context: C, result: Result<T, E>) -> Option<T>
where
    C: std::fmt::Display,
    E: std::fmt::Display,
{
    result.map_or_else(
        |e| {
            // tracing::error!("Failed to parse from database in {context}: {e}");
            debug_assert!(false, "Failed to parse from database in {context}: {e}");
            None
        },
        Some,
    )
}
