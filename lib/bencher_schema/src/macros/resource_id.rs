macro_rules! fn_eq_resource_id {
    ($table:ident) => {
        pub fn eq_resource_id(
            resource_id: &bencher_json::ResourceId,
        ) -> Result<
            Box<
                dyn diesel::BoxableExpression<
                        $crate::schema::$table::table,
                        diesel::sqlite::Sqlite,
                        SqlType = diesel::sql_types::Bool,
                    >,
            >,
            dropshot::HttpError,
        > {
            Ok(
                match resource_id.try_into().map_err(|e| {
                    $crate::error::issue_error(
                        "Failed to parse resource ID",
                        "Failed to parse resource ID.",
                        e,
                    )
                })? {
                    bencher_json::ResourceIdKind::Uuid(uuid) => {
                        Box::new($crate::schema::$table::uuid.eq(uuid.to_string()))
                    },
                    bencher_json::ResourceIdKind::Slug(slug) => {
                        Box::new($crate::schema::$table::slug.eq(slug.to_string()))
                    },
                },
            )
        }
    };
}

pub(crate) use fn_eq_resource_id;

macro_rules! fn_from_resource_id {
    ($parent:ident, $parent_type:ty, $table:ident, $resource:ident) => {
        pub fn from_resource_id(
            conn: &mut $crate::context::DbConnection,
            parent: $parent_type,
            resource_id: &bencher_json::ResourceId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::$parent.eq(parent))
                .filter(Self::eq_resource_id(resource_id)?)
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    (parent, resource_id)
                ))
        }
    };
    ($table:ident, $resource:ident) => {
        pub fn from_resource_id(
            conn: &mut $crate::context::DbConnection,
            resource_id: &bencher_json::ResourceId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(Self::eq_resource_id(resource_id)?)
                .first::<Self>(conn)
                .map_err($crate::error::resource_not_found_err!(
                    $resource,
                    resource_id
                ))
        }
    };
}

pub(crate) use fn_from_resource_id;
