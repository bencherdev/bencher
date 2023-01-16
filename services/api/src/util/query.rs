macro_rules! fn_get_id {
    ($table:ident) => {
        pub fn get_id<U>(
            conn: &mut diesel::SqliteConnection,
            uuid: &U,
        ) -> Result<i32, crate::ApiError>
        where
            U: ToString,
        {
            schema::$table::table
                .filter(schema::$table::uuid.eq(uuid.to_string()))
                .select(schema::$table::id)
                .first(conn)
                .map_err(crate::error::api_error!())
        }
    };
}

pub(crate) use fn_get_id;
