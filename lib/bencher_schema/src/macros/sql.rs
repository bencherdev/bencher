diesel::define_sql_function! {
    /// `SQLite` `last_insert_rowid()` — returns the rowid of the most recent INSERT.
    fn last_insert_rowid() -> diesel::sql_types::Integer;
}
