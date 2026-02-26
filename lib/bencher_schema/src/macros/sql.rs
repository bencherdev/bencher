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
