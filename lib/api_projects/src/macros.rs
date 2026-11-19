/// Filter `query` to the rows of `table` that `name_id` names in the project, as
/// `find_name_id` resolves it: a slug that no row of the project holds is matched as a name.
macro_rules! filter_name_id {
    ($query:ident, $table:ident, $name_id:ident, $project_id:expr) => {
        match $name_id {
            bencher_json::NameId::Uuid(uuid) => {
                $query = $query.filter(bencher_schema::schema::$table::uuid.eq(uuid.to_string()));
            },
            bencher_json::NameId::Slug(slug) => {
                use diesel::BoolExpressionMethods as _;
                let slug_held = bencher_schema::schema::$table::table
                    .filter(bencher_schema::schema::$table::project_id.eq($project_id))
                    .filter(bencher_schema::schema::$table::slug.eq(slug.to_string()))
                    .select(bencher_schema::schema::$table::id)
                    .into_boxed();
                $query = $query.filter(
                    bencher_schema::schema::$table::slug
                        .eq(slug.to_string())
                        .or(bencher_schema::schema::$table::name
                            .eq(slug.to_string())
                            .and(diesel::dsl::not(diesel::dsl::exists(slug_held)))),
                );
            },
            bencher_json::NameId::Name(name) => {
                $query = $query.filter(bencher_schema::schema::$table::name.eq(name.to_string()));
            },
        }
    };
}

pub(crate) use filter_name_id;

macro_rules! filter_branch_name_id {
    ($query:ident, $name_id:ident, $project_id:expr) => {
        crate::macros::filter_name_id!($query, branch, $name_id, $project_id)
    };
}

pub(crate) use filter_branch_name_id;

macro_rules! filter_testbed_name_id {
    ($query:ident, $name_id:ident, $project_id:expr) => {
        crate::macros::filter_name_id!($query, testbed, $name_id, $project_id)
    };
}

pub(crate) use filter_testbed_name_id;

macro_rules! filter_measure_name_id {
    ($query:ident, $name_id:ident, $project_id:expr) => {
        crate::macros::filter_name_id!($query, measure, $name_id, $project_id)
    };
}

pub(crate) use filter_measure_name_id;
