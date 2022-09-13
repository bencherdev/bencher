use uuid::Uuid;

use crate::ApiError;

pub enum ResourceId {
    Uuid(Uuid),
    Slug(String),
}

impl TryFrom<&bencher_json::ResourceId> for ResourceId {
    type Error = ApiError;

    fn try_from(resource_id: &bencher_json::ResourceId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::try_parse(&resource_id.0) {
            return Ok(ResourceId::Uuid(uuid));
        }
        let slug = slug::slugify(&resource_id.0);
        if resource_id.0 == slug {
            return Ok(ResourceId::Slug(slug));
        }
        Err(ApiError::ResourceId)
    }
}

macro_rules! fn_resource_id {
    ($table:ident) => {
        fn resource_id(
            resource_id: &bencher_json::ResourceId,
        ) -> Result<
            Box<
                dyn diesel::BoxableExpression<
                    crate::schema::$table::table,
                    diesel::sqlite::Sqlite,
                    SqlType = diesel::sql_types::Bool,
                >,
            >,
            crate::error::ApiError,
        > {
            Ok(match resource_id.try_into()? {
                crate::util::resource_id::ResourceId::Uuid(uuid) => {
                    Box::new(crate::schema::$table::uuid.eq(uuid.to_string()))
                },
                crate::util::resource_id::ResourceId::Slug(slug) => {
                    Box::new(crate::schema::$table::slug.eq(slug.clone()))
                },
            })
        }
    };
}

pub(crate) use fn_resource_id;
