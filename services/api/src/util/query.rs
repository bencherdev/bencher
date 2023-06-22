macro_rules! fn_get {
    ($table:ident) => {
        pub fn get(
            conn: &mut crate::context::DbConnection,
            id: i32,
        ) -> Result<Self, crate::ApiError> {
            schema::$table::table
                .filter(schema::$table::id.eq(id))
                .first(conn)
                .map_err(crate::error::api_error!())
        }
    };
}

pub(crate) use fn_get;

macro_rules! fn_get_id {
    ($table:ident) => {
        pub fn get_id<U>(
            conn: &mut crate::context::DbConnection,
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
