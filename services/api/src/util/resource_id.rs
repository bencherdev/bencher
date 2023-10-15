use std::str::FromStr;

use bencher_json::Slug;
use dropshot::HttpError;
use uuid::Uuid;

use crate::error::bad_request_error;

pub enum ResourceId {
    Uuid(Uuid),
    Slug(Slug),
}

impl TryFrom<&bencher_json::ResourceId> for ResourceId {
    type Error = HttpError;

    fn try_from(resource_id: &bencher_json::ResourceId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(resource_id.as_ref()) {
            Ok(ResourceId::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(resource_id.as_ref()) {
            Ok(ResourceId::Slug(slug))
        } else {
            Err(bad_request_error(format!(
                "Failed to parse resource ID: {resource_id}"
            )))
        }
    }
}

macro_rules! fn_resource_id {
    ($table:ident) => {
        #[allow(unused_qualifications)]
        pub fn resource_id(
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
            Ok(match resource_id.try_into()? {
                crate::util::resource_id::ResourceId::Uuid(uuid) => {
                    Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                crate::util::resource_id::ResourceId::Slug(slug) => {
                    Box::new(crate::schema::$table::slug.eq(slug.to_string()))
                },
            })
        }
    };
}

pub(crate) use fn_resource_id;
