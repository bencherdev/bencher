macro_rules! typed_slug {
    ($name:ident) => {
        #[typeshare::typeshare]
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            derive_more::Display,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[cfg_attr(feature = "schema", derive(JsonSchema))]
        #[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
        #[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
        pub struct $name($crate::Slug);

        impl From<$name> for $crate::Slug {
            fn from(slug: $name) -> Self {
                slug.0
            }
        }

        impl From<$crate::Slug> for $name {
            fn from(slug: $crate::Slug) -> Self {
                Self(slug)
            }
        }

        impl From<$name> for crate::ResourceId {
            fn from(slug: $name) -> Self {
                slug.0.into()
            }
        }

        impl From<$name> for crate::NameId<$crate::Slug> {
            fn from(slug: $name) -> Self {
                slug.0.into()
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::ValidError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self($crate::Slug::from_str(s)?))
            }
        }

        impl AsRef<$crate::Slug> for $name {
            fn as_ref(&self) -> &$crate::Slug {
                &self.0
            }
        }

        impl bencher_valid::NamedSlug for $name {
            fn slug(&self) -> $crate::Slug {
                self.0.clone()
            }
        }

        $crate::typed_db::typed_db!($name);
    };
}

pub(crate) use typed_slug;
