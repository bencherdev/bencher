macro_rules! fn_eq_resource_id {
    ($table:ident) => {
        #[allow(unused_qualifications)]
        pub fn eq_resource_id(
            resource_id: &bencher_json::ResourceId,
        ) -> Result<
            Box<
                dyn diesel::BoxableExpression<
                    crate::schema::$table::table,
                    diesel::sqlite::Sqlite,
                    SqlType = diesel::sql_types::Bool,
                >,
            >,
            dropshot::HttpError,
        > {
            Ok(
                match resource_id.try_into().map_err(|e| {
                    crate::error::issue_error(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to parse resource ID",
                        "Failed to parse resource ID.",
                        e,
                    )
                })? {
                    bencher_json::ResourceIdKind::Uuid(uuid) => {
                        Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                    },
                    bencher_json::ResourceIdKind::Slug(slug) => {
                        Box::new(crate::schema::$table::slug.eq(slug.to_string()))
                    },
                },
            )
        }
    };
}

pub(crate) use fn_eq_resource_id;

macro_rules! fn_from_resource_id {
    // The `root` parameter is just a kludge to distinguish between top level and project level resources
    ($table:ident, $resource:ident, $root:expr) => {
        #[allow(unused_qualifications)]
        pub fn from_resource_id(
            conn: &mut crate::context::DbConnection,
            resource_id: &bencher_json::ResourceId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(Self::eq_resource_id(resource_id)?)
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!(
                    $resource,
                    resource_id
                ))
        }
    };
    ($table:ident, $resource:ident) => {
        #[allow(unused_qualifications)]
        pub fn from_resource_id(
            conn: &mut crate::context::DbConnection,
            project_id: crate::model::project::ProjectId,
            resource_id: &bencher_json::ResourceId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::project_id.eq(project_id))
                .filter(Self::eq_resource_id(resource_id)?)
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!(
                    $resource,
                    (project_id, resource_id)
                ))
        }
    };
}

pub(crate) use fn_from_resource_id;
