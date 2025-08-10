// This exists solely to export a `Uuid` type to Typescript that then aliases to `string`.
#[typeshare::typeshare]
#[expect(dead_code)]
pub struct Uuid(pub uuid::Uuid);

macro_rules! typed_uuid {
    ($uuid:ident) => {
        #[typeshare::typeshare]
        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            Hash,
            derive_more::Display,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[cfg_attr(feature = "schema", derive(JsonSchema))]
        #[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
        #[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
        pub struct $uuid(uuid::Uuid);

        impl From<$uuid> for uuid::Uuid {
            fn from(uuid: $uuid) -> Self {
                uuid.0
            }
        }

        impl From<uuid::Uuid> for $uuid {
            fn from(uuid: uuid::Uuid) -> Self {
                Self(uuid)
            }
        }

        impl std::str::FromStr for $uuid {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(uuid::Uuid::parse_str(s)?))
            }
        }

        impl AsRef<uuid::Uuid> for $uuid {
            fn as_ref(&self) -> &uuid::Uuid {
                &self.0
            }
        }

        impl $uuid {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }

        $crate::typed_db::typed_db!($uuid);
    };
}

pub(crate) use typed_uuid;
