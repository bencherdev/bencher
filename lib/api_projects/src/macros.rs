macro_rules! filter_name_id {
    ($name:ident, $query:ident, $table:ident, $name_id:ident) => {
        match $name_id {
            bencher_json::NamedId::Uuid(uuid) => {
                $query = $query.filter(bencher_schema::schema::$table::uuid.eq(uuid.to_string()));
            },
            bencher_json::NamedId::Slug(slug) => {
                $query = $query.filter(bencher_schema::schema::$table::slug.eq(slug.to_string()));
            },
            bencher_json::NamedId::Name(name) => {
                $query = $query.filter(bencher_schema::schema::$table::name.eq(name.to_string()));
            },
        }
    };
}

pub(crate) use filter_name_id;

macro_rules! filter_branch_name_id {
    ($query:ident, $name_id:ident) => {
        crate::macros::filter_name_id!(BranchName, $query, branch, $name_id)
    };
}

pub(crate) use filter_branch_name_id;

macro_rules! filter_testbed_name_id {
    ($query:ident, $name_id:ident) => {
        crate::macros::filter_name_id!(ResourceName, $query, testbed, $name_id)
    };
}

pub(crate) use filter_testbed_name_id;

macro_rules! filter_measure_name_id {
    ($query:ident, $name_id:ident) => {
        crate::macros::filter_name_id!(ResourceName, $query, measure, $name_id)
    };
}

pub(crate) use filter_measure_name_id;
