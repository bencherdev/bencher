// The `define_sql_function!` macro generates structs with `pub(in crate)` fields
// for function parameters, which triggers the `field_scoped_visibility_modifiers` lint.
#![expect(clippy::field_scoped_visibility_modifiers)]

diesel::define_sql_function! {
    /// `SQLite` `last_insert_rowid()` — returns the rowid of the most recent INSERT on the same
    /// connection.
    ///
    /// Must be called within a transaction to ensure no interleaving INSERT corrupts the result.
    ///
    /// Returns `Integer` (i32) which matches all typed IDs in this codebase (`typed_id!` wraps
    /// i32). `SQLite`'s actual return is i64, but overflow is not a concern for this codebase's
    /// scale.
    fn last_insert_rowid() -> diesel::sql_types::Integer;
}

diesel::define_sql_function! {
    /// `SQLite` scalar `MIN(a, b)` — returns the smaller of two integer values.
    ///
    /// Used to clamp SQL-level arithmetic to prevent silent overflow beyond `i32::MAX`,
    /// which `SQLite` would store as i64 but Diesel reads back via `sqlite3_value_int()`
    /// (silent truncation).
    fn min(a: diesel::sql_types::Integer, b: diesel::sql_types::Integer) -> diesel::sql_types::Integer;
}
