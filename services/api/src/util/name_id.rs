use std::str::FromStr;

use bencher_json::{NonEmpty, Slug};
use dropshot::HttpError;
use uuid::Uuid;

use crate::error::bad_request_error;

pub enum NameId {
    Uuid(Uuid),
    Slug(Slug),
    Name(NonEmpty),
}

impl TryFrom<&bencher_json::NameId> for NameId {
    type Error = HttpError;

    fn try_from(name_id: &bencher_json::NameId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(name_id.as_ref()) {
            Ok(NameId::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(name_id.as_ref()) {
            Ok(NameId::Slug(slug))
        } else if let Ok(non_empty) = NonEmpty::from_str(name_id.as_ref()) {
            Ok(NameId::Name(non_empty))
        } else {
            Err(bad_request_error(format!(
                "Failed to parse name ID: {name_id}"
            )))
        }
    }
}

macro_rules! fn_name_id {
    ($table:ident) => {
        #[allow(unused_qualifications)]
        pub fn name_id(
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
            Ok(match name_id.try_into()? {
                crate::util::name_id::NameId::Uuid(uuid) => {
                    Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                crate::util::name_id::NameId::Slug(slug) => {
                    Box::new(crate::schema::$table::slug.eq(slug.as_ref()))
                },
                crate::util::name_id::NameId::Name(name) => {
                    Box::new(crate::schema::$table::name.eq(name.as_ref()))
                },
            })
        }
    };
}

pub(crate) use fn_name_id;

macro_rules! fn_from_name_id {
    // The `root` parameter is just a kludge to distinguish between top level and project level resources
    ($table:ident, $resource:ident, $root:expr) => {
        #[allow(unused_qualifications)]
        pub fn from_name_id(conn: &mut DbConnection, name_id: &NameId) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(Self::name_id(name_id)?)
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!($resource, name_id))
        }
    };
    ($table:ident, $resource:ident) => {
        #[allow(unused_qualifications)]
        pub fn from_name_id(
            conn: &mut DbConnection,
            project_id: ProjectId,
            name_id: &NameId,
        ) -> Result<Self, HttpError> {
            schema::$table::table
                .filter(schema::$table::project_id.eq(project_id))
                .filter(Self::name_id(name_id)?)
                .first::<Self>(conn)
                .map_err(crate::error::resource_not_found_err!(
                    $resource,
                    (project_id, name_id)
                ))
        }
    };
}

pub(crate) use fn_from_name_id;
