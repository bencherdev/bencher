macro_rules! fn_eq_name_id {
    ($name:ident, $table:ident, $name_id:ident) => {
        pub fn eq_name_id(
            name_id: &bencher_json::$name_id,
        ) -> Box<
            dyn diesel::BoxableExpression<
                    $crate::schema::$table::table,
                    diesel::sqlite::Sqlite,
                    SqlType = diesel::sql_types::Bool,
                >,
        > {
            match name_id {
                bencher_json::NameId::Uuid(uuid) => {
                    Box::new($crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                bencher_json::NameId::Slug(slug) => {
                    Box::new($crate::schema::$table::slug.eq(slug.to_string()))
                },
                bencher_json::NameId::Name(name) => {
                    Box::new($crate::schema::$table::name.eq(name.to_string()))
                },
            }
        }
    };
}

pub(crate) use fn_eq_name_id;

macro_rules! fn_from_name_id {
    ($table:ident, $resource:ident, $name_id:ident) => {
        pub fn from_name_id(
            conn: &mut crate::context::DbConnection,
            project_id: crate::model::project::ProjectId,
            name_id: &bencher_json::$name_id,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::project_id.eq(project_id))
                .filter(Self::eq_name_id(name_id))
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!(
                    $resource,
                    (project_id, name_id)
                ))
        }
    };
}

pub(crate) use fn_from_name_id;
