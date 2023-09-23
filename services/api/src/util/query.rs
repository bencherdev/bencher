macro_rules! fn_get {
    ($table:ident) => {
        #[allow(unused_qualifications)]
        pub fn get<Id>(
            conn: &mut crate::context::DbConnection,
            id: Id,
        ) -> Result<Self, crate::ApiError>
        where
            Id: Into<i32>,
        {
            schema::$table::table
                .filter(schema::$table::id.eq(id.into()))
                .first(conn)
                .map_err(crate::error::ApiError::from)
        }
    };
}

pub(crate) use fn_get;

macro_rules! fn_get_id {
    ($table:ident, $id:ident) => {
        #[allow(unused_qualifications)]
        pub fn get_id<U>(
            conn: &mut crate::context::DbConnection,
            uuid: &U,
        ) -> Result<$id, crate::ApiError>
        where
            U: ToString,
        {
            schema::$table::table
                .filter(schema::$table::uuid.eq(uuid.to_string()))
                .select(schema::$table::id)
                .first(conn)
                .map_err(crate::error::ApiError::from)
        }
    };
}

pub(crate) use fn_get_id;
