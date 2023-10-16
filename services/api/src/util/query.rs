macro_rules! fn_get {
    ($table:ident, $id:ident) => {
        #[allow(unused_qualifications)]
        pub fn get(
            conn: &mut crate::context::DbConnection,
            id: $id,
        ) -> Result<Self, dropshot::HttpError> {
            schema::$table::table
                .filter(schema::$table::id.eq(id))
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table with ID ({id})",
                        table = stringify!($table)
                    );
                    crate::error::issue_error(http::StatusCode::NOT_FOUND, &message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get;

macro_rules! fn_get_id {
    ($table:ident, $id:ident, $uuid:ident) => {
        #[allow(unused_qualifications)]
        pub fn get_id(
            conn: &mut crate::context::DbConnection,
            uuid: $uuid,
        ) -> Result<$id, dropshot::HttpError> {
            schema::$table::table
                .filter(schema::$table::uuid.eq(uuid))
                .select(schema::$table::id)
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table with UUID ({uuid})",
                        table = stringify!($table)
                    );
                    crate::error::issue_error(http::StatusCode::NOT_FOUND, &message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get_id;

macro_rules! fn_get_uuid {
    ($table:ident, $id:ident, $uuid:ident) => {
        #[allow(unused_qualifications)]
        pub fn get_uuid(
            conn: &mut crate::context::DbConnection,
            id: $id,
        ) -> Result<$uuid, dropshot::HttpError> {
            schema::$table::table
                .filter(schema::$table::id.eq(id))
                .select(schema::$table::uuid)
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table for for UUID with ID ({id})",
                        table = stringify!($table)
                    );
                    crate::error::issue_error(http::StatusCode::NOT_FOUND, &message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get_uuid;
