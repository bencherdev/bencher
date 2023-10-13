use std::{fmt, str::FromStr};

use bencher_valid::{NonEmpty, Slug, Url};
use chrono::{DateTime, Utc};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

use crate::OrganizationUuid;

pub mod alert;
pub mod benchmark;
pub mod boundary;
pub mod branch;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod report;
pub mod testbed;
pub mod threshold;

crate::typed_uuid::typed_uuid!(ProjectUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewProject {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjects(pub Vec<JsonProject>);

crate::from_vec!(JsonProjects[JsonProject]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: ProjectUuid,
    pub organization: OrganizationUuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub url: Option<Url>,
    pub visibility: Visibility,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

impl fmt::Display for JsonProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// Unfortunately, we have to use a complex, custom type and deserializer here.
// Due to some limitations in JSON Schema, we can't just use an `Option<Option<Url>>`.
// We need to be able to disambiguate between:
// - a missing `url` key
// - a `url` key with the value of `null`
// If we were writing our own client, we could do something like this:
// https://docs.rs/serde_with/latest/serde_with/rust/double_option/index.html
// However, we need `progenitor` to create a client that can accommodate both use cases.
// Just isolating the variants to the `url` field doesn't work either
// because `dropshot` doesn't like a flattened and untagged inner struct enum.
// So we are left with this solution, a top-level, untagged enum.
// In the future, avoid this by not having nullable fields in the API that need to be individually modified.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdateProject {
    Patch(JsonProjectPatch),
    Null(JsonProjectPatchNull),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatch {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatchNull {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: (),
    pub visibility: Option<Visibility>,
}

impl<'de> Deserialize<'de> for JsonUpdateProject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const NAME_FIELD: &str = "name";
        const SLUG_FIELD: &str = "slug";
        const URL_FIELD: &str = "url";
        const VISIBILITY_FIELD: &str = "visibility";
        const FIELDS: &[&str] = &[NAME_FIELD, SLUG_FIELD, URL_FIELD, VISIBILITY_FIELD];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Slug,
            Url,
            Visibility,
        }

        struct UpdateProjectVisitor;

        impl<'de> Visitor<'de> for UpdateProjectVisitor {
            type Value = JsonUpdateProject;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdateProject")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut name = None;
                let mut slug = None;
                let mut url = None;
                let mut visibility = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field(NAME_FIELD));
                            }
                            name = Some(map.next_value()?);
                        },
                        Field::Slug => {
                            if slug.is_some() {
                                return Err(serde::de::Error::duplicate_field(SLUG_FIELD));
                            }
                            slug = Some(map.next_value()?);
                        },
                        Field::Url => {
                            if url.is_some() {
                                return Err(serde::de::Error::duplicate_field(URL_FIELD));
                            }
                            url = Some(map.next_value()?);
                        },
                        Field::Visibility => {
                            if visibility.is_some() {
                                return Err(serde::de::Error::duplicate_field(VISIBILITY_FIELD));
                            }
                            visibility = Some(map.next_value()?);
                        },
                    }
                }

                Ok(match url {
                    Some(Some(url)) => Self::Value::Patch(JsonProjectPatch {
                        name,
                        slug,
                        url: Some(url),
                        visibility,
                    }),
                    Some(None) => Self::Value::Null(JsonProjectPatchNull {
                        name,
                        slug,
                        url: (),
                        visibility,
                    }),
                    None => Self::Value::Patch(JsonProjectPatch {
                        name,
                        slug,
                        url: None,
                        visibility,
                    }),
                })
            }
        }

        deserializer.deserialize_struct("JsonUpdateProject", FIELDS, UpdateProjectVisitor)
    }
}

impl JsonUpdateProject {
    pub fn visibility(&self) -> Option<Visibility> {
        match self {
            Self::Patch(patch) => patch.visibility,
            Self::Null(patch) => patch.visibility,
        }
    }
}

const PUBLIC_INT: i32 = 0;
#[cfg(feature = "plus")]
const PRIVATE_INT: i32 = 1;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum Visibility {
    #[default]
    Public = PUBLIC_INT,
    #[cfg(feature = "plus")]
    Private = PRIVATE_INT,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

#[cfg(feature = "db")]
mod visibility {
    #[cfg(feature = "plus")]
    use super::PRIVATE_INT;
    use super::{Visibility, PUBLIC_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum VisibilityError {
        #[error("Invalid visibility value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Visibility
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Public => PUBLIC_INT.to_sql(out),
                #[cfg(feature = "plus")]
                Self::Private => PRIVATE_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Visibility
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                PUBLIC_INT => Ok(Self::Public),
                #[cfg(feature = "plus")]
                PRIVATE_INT => Ok(Self::Private),
                value => Err(Box::new(VisibilityError::Invalid(value))),
            }
        }
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Display)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProjectPermission {
    #[display(fmt = "view")]
    View,
    #[display(fmt = "create")]
    Create,
    #[display(fmt = "edit")]
    Edit,
    #[display(fmt = "delete")]
    Delete,
    #[display(fmt = "manage")]
    Manage,
    #[display(fmt = "view_role")]
    ViewRole,
    #[display(fmt = "create_role")]
    CreateRole,
    #[display(fmt = "edit_role")]
    EditRole,
    #[display(fmt = "delete_role")]
    DeleteRole,
}

pub const VIEWER_ROLE: &str = "viewer";
pub const DEVELOPER_ROLE: &str = "developer";
pub const MAINTAINER_ROLE: &str = "maintainer";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
#[serde(rename_all = "snake_case")]
pub enum ProjectRole {
    // TODO Team Management
    // Viewer,
    // Developer,
    Maintainer,
}

impl FromStr for ProjectRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // TODO Team Management
            // MEMBER_ROLE => Ok(Self::Member),
            MAINTAINER_ROLE => Ok(Self::Maintainer),
            _ => Err(s.into()),
        }
    }
}

impl fmt::Display for ProjectRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                // TODO Team Management
                // Self::Member => MEMBER_ROLE,
                Self::Maintainer => MAINTAINER_ROLE,
            }
        )
    }
}

#[cfg(feature = "db")]
mod organization_role {
    use super::{ProjectRole, MAINTAINER_ROLE};

    #[derive(Debug, thiserror::Error)]
    pub enum ProjectRoleError {
        #[error("Invalid project role value: {0}")]
        Invalid(String),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for ProjectRole
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Maintainer => out.set_value(MAINTAINER_ROLE.to_owned()),
            }
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for ProjectRole
    where
        DB: diesel::backend::Backend,
        String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let role = String::from_sql(bytes)?;
            match role.as_str() {
                MAINTAINER_ROLE => Ok(Self::Maintainer),
                _ => Err(Box::new(ProjectRoleError::Invalid(role))),
            }
        }
    }
}
