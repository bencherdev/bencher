macro_rules! typed_slug {
    ($slug:ident) => {
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
        pub struct $slug($crate::Slug);

        impl From<$slug> for $crate::Slug {
            fn from(slug: $slug) -> Self {
                slug.0
            }
        }

        impl From<$crate::Slug> for $slug {
            fn from(slug: $crate::Slug) -> Self {
                Self(slug)
            }
        }

        impl From<$slug> for crate::ResourceId {
            fn from(slug: $slug) -> Self {
                slug.0.into()
            }
        }

        // impl From<$slug> for crate::NameId<$crate::Slug> {
        //     fn from(slug: $slug) -> Self {
        //         slug.0.into()
        //     }
        // }

        impl std::str::FromStr for $slug {
            type Err = $crate::ValidError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self($crate::Slug::from_str(s)?))
            }
        }

        impl AsRef<$crate::Slug> for $slug {
            fn as_ref(&self) -> &$crate::Slug {
                &self.0
            }
        }

        $crate::typed_db::typed_db!($slug);
    };
    ($slug:ident, $name:ident) => {
        $crate::typed_slug::typed_slug!($slug);

        impl From<$slug> for $name {
            fn from(slug: $slug) -> Self {
                $crate::Slug::from(slug).into()
            }
        }
    };
}

pub(crate) use typed_slug;
