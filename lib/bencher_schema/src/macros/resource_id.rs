macro_rules! fn_eq_resource_id {
    ($table:ident, $resource_id:ident) => {
        pub fn eq_resource_id(
            resource_id: &bencher_json::$resource_id,
        ) -> Box<
            dyn diesel::BoxableExpression<
                    $crate::schema::$table::table,
                    diesel::sqlite::Sqlite,
                    SqlType = diesel::sql_types::Bool,
                >,
        > {
            match resource_id {
                bencher_json::ResourceId::Uuid(uuid) => {
                    Box::new($crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                bencher_json::ResourceId::Slug(slug) => {
                    Box::new($crate::schema::$table::slug.eq(slug.to_string()))
                },
            }
        }
    };
}

pub(crate) use fn_eq_resource_id;

macro_rules! fn_from_resource_id {
    ($parent:ident, $parent_type:ty, $table:ident, $resource:ident, $resource_id:ident) => {
        pub fn from_resource_id(
            conn: &mut $crate::context::DbConnection,
            parent: $parent_type,
            resource_id: &bencher_json::$resource_id,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::$parent.eq(parent))
                .filter(Self::eq_resource_id(resource_id))
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    (parent, resource_id)
                ))
        }
    };
    ($table:ident, $resource:ident, $resource_id:ident) => {
        pub fn from_resource_id(
            conn: &mut $crate::context::DbConnection,
            resource_id: &bencher_json::$resource_id,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(Self::eq_resource_id(resource_id))
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    resource_id
                ))
        }
    };
}

pub(crate) use fn_from_resource_id;

macro_rules! fn_from_resource_id_not_deleted {
    ($table:ident, $resource:ident, $resource_id:ident) => {
        pub fn from_resource_id(
            conn: &mut $crate::context::DbConnection,
            resource_id: &bencher_json::$resource_id,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(Self::eq_resource_id(resource_id))
                .filter(schema::$table::deleted.is_null())
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    resource_id
                ))
        }
    };
}

pub(crate) use fn_from_resource_id_not_deleted;
