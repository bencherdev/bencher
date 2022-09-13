macro_rules! fn_resource_id {
    ($table:ident) => {
        fn resource_id(
            resource_id: &bencher_json::ResourceId,
        ) -> Box<
            dyn diesel::BoxableExpression<
                crate::schema::$table::table,
                diesel::sqlite::Sqlite,
                SqlType = diesel::sql_types::Bool,
            >,
        > {
            match resource_id {
                bencher_json::ResourceId::Uuid(uuid) => {
                    Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                bencher_json::ResourceId::Slug(slug) => {
                    Box::new(crate::schema::$table::slug.eq(slug.clone()))
                },
            }
        }
    };
}

pub(crate) use fn_resource_id;
