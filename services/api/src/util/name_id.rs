macro_rules! fn_eq_name_id {
    ($name:ident, $table:ident) => {
        #[allow(unused_qualifications)]
        pub fn eq_name_id(
            name_id: &bencher_json::NameId,
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
                match name_id.try_into().map_err(|e| {
                    crate::error::issue_error(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to parse name ID",
                        "Failed to parse name ID.",
                        e,
                    )
                })? {
                    bencher_json::NameIdKind::Uuid(uuid) => {
                        Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                    },
                    bencher_json::NameIdKind::Slug(slug) => {
                        Box::new(crate::schema::$table::slug.eq(slug.to_string()))
                    },
                    bencher_json::NameIdKind::Name(name) => {
                        let name: bencher_json::$name = name;
                        Box::new(crate::schema::$table::name.eq(name.to_string()))
                    },
                },
            )
        }
    };
}

pub(crate) use fn_eq_name_id;

macro_rules! fn_from_name_id {
    ($table:ident, $resource:ident) => {
        #[allow(unused_qualifications)]
        pub fn from_name_id(
            conn: &mut crate::context::DbConnection,
            project_id: crate::model::project::ProjectId,
            name_id: &bencher_json::NameId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::project_id.eq(project_id))
                .filter(Self::eq_name_id(name_id)?)
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!(
                    $resource,
                    (project_id, name_id)
                ))
        }
    };
}

pub(crate) use fn_from_name_id;

macro_rules! filter_name_id {
    ($name:ident, $query:ident, $table:ident, $name_id:ident) => {
        #[allow(unused_qualifications)]
        match $name_id.try_into().map_err(|e| {
            crate::error::issue_error(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse name ID",
                "Failed to parse name ID.",
                e,
            )
        })? {
            bencher_json::NameIdKind::Uuid(uuid) => {
                $query = $query.filter(schema::$table::uuid.eq(uuid.to_string()));
            },
            bencher_json::NameIdKind::Slug(slug) => {
                $query = $query.filter(schema::$table::slug.eq(slug.to_string()));
            },
            bencher_json::NameIdKind::Name(name) => {
                let name: bencher_json::$name = name;
                $query = $query.filter(schema::$table::name.eq(name.to_string()));
            },
        }
    };
}

pub(crate) use filter_name_id;

macro_rules! filter_branch_name_id {
    ($query:ident, $name_id:ident) => {
        crate::util::name_id::filter_name_id!(BranchName, $query, branch, $name_id)
    };
}

pub(crate) use filter_branch_name_id;

macro_rules! filter_testbed_name_id {
    ($query:ident, $name_id:ident) => {
        crate::util::name_id::filter_name_id!(ResourceName, $query, testbed, $name_id)
    };
}

pub(crate) use filter_testbed_name_id;

macro_rules! filter_measure_name_id {
    ($query:ident, $name_id:ident) => {
        crate::util::name_id::filter_name_id!(ResourceName, $query, measure, $name_id)
    };
}

pub(crate) use filter_measure_name_id;
