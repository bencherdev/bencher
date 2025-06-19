macro_rules! fn_get {
    ($table:ident, $id:ident) => {
        #[allow(unused_qualifications)]
        pub fn get(
            conn: &mut $crate::context::DbConnection,
            id: $id,
        ) -> Result<Self, dropshot::HttpError> {
            use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
            $crate::schema::$table::table
                .filter($crate::schema::$table::id.eq(id))
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table with ID ({id})",
                        table = stringify!($table)
                    );
                    $crate::error::issue_error(&message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get;

macro_rules! fn_get_id {
    ($table:ident, $id:ident, $uuid:ident) => {
        #[allow(unused_qualifications)]
        pub fn get_id(
            conn: &mut $crate::context::DbConnection,
            uuid: $uuid,
        ) -> Result<$id, dropshot::HttpError> {
            use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
            $crate::schema::$table::table
                .filter($crate::schema::$table::uuid.eq(uuid))
                .select($crate::schema::$table::id)
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table with UUID ({uuid})",
                        table = stringify!($table)
                    );
                    $crate::error::issue_error(&message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get_id;

macro_rules! fn_get_uuid {
    ($table:ident, $id:ident, $uuid:ident) => {
        #[allow(unused_qualifications)]
        pub fn get_uuid(
            conn: &mut $crate::context::DbConnection,
            id: $id,
        ) -> Result<$uuid, dropshot::HttpError> {
            use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
            $crate::schema::$table::table
                .filter($crate::schema::$table::id.eq(id))
                .select($crate::schema::$table::uuid)
                .first(conn)
                .map_err(|e| {
                    let message = format!(
                        "Failed to query {table} table for for UUID with ID ({id})",
                        table = stringify!($table)
                    );
                    $crate::error::issue_error(&message, &message, e)
                })
        }
    };
}

pub(crate) use fn_get_uuid;

macro_rules! fn_from_uuid {
    ($parent:ident, $parent_type:ty, $table:ident, $uuid:ident, $resource:ident) => {
        #[allow(unused_qualifications)]
        pub fn from_uuid(
            conn: &mut DbConnection,
            parent: $parent_type,
            uuid: $uuid,
        ) -> Result<Self, HttpError> {
            use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
            $crate::schema::$table::table
                .filter($crate::schema::$table::$parent.eq(parent))
                .filter($crate::schema::$table::uuid.eq(uuid))
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    (parent, uuid)
                ))
        }
    };
    ($table:ident, $uuid:ident, $resource:ident) => {
        fn_from_uuid!(project_id, ProjectId, $table, $uuid, $resource);
    };
}

pub(crate) use fn_from_uuid;
