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
        ) -> Result<$id, dropshot::HttpError>
        where
            U: ToString,
        {
            let uuid_str = uuid.to_string();
            schema::$table::table
                .filter(schema::$table::uuid.eq(&uuid_str))
                .select(schema::$table::id)
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table for uuid {uuid_str}",
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
                .map_err(crate::error::not_found_error)
        }
    };
}

pub(crate) use fn_get_uuid;

// pub(crate) use fn_from_resource_id;

// macro_rules! fn_get_uuid {
//     ($parent:ident, $table:ident, $id:ident, $uuid:ident) => {
//         #[allow(unused_qualifications)]
//         pub fn from_resource_id(
//             conn: &mut DbConnection,
//             parent: $parent,
//             benchmark: &ResourceId,
//         ) -> Result<Self, HttpError> {
//             crate::resource_id::fn_resource_id!($table);

//             schema::$table::table
//                 .filter(schema::benchmark::project_id.eq(project_id))
//                 .filter(resource_id(benchmark)?)
//                 .first::<Self>(conn)
//                 .map_err(resource_not_found_err!(Benchmark, benchmark.clone()))
//         }

//         pub fn get_uuid(
//             conn: &mut crate::context::DbConnection,
//             id: $id,
//         ) -> Result<$uuid, dropshot::HttpError> {
//             schema::$table::table
//                 .filter(schema::$table::id.eq(id))
//                 .select(schema::$table::uuid)
//                 .first(conn)
//                 .map_err(crate::error::not_found_error)
//         }
//     };
// }

// pub(crate) use fn_from_resource_id;
