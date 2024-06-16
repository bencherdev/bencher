use std::fmt;

use bencher_valid::{DateTime, ResourceName, Slug};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

pub mod member;
pub mod plan;
pub mod usage;

crate::typed_uuid::typed_uuid!(OrganizationUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewOrganization {
    /// The name of the organization.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The preferred slug for the organization.
    /// If not provided, the slug will be generated from the name.
    /// If the provided or generated slug is already in use, a unique slug will be generated.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganizations(pub Vec<JsonOrganization>);

crate::from_vec!(JsonOrganizations[JsonOrganization]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganization {
    pub uuid: OrganizationUuid,
    pub name: ResourceName,
    pub slug: Slug,
    #[cfg(feature = "plus")]
    pub license: Option<bencher_valid::Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

// Unfortunately, we have to use a complex, custom type and deserializer here.
// Due to some limitations in JSON Schema, we can't just use an `Option<Option<Jwt>>`.
// We need to be able to disambiguate between:
// - a missing `license` key
// - a `license` key with the value of `null`
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
pub enum JsonUpdateOrganization {
    Patch(JsonOrganizationPatch),
    Null(JsonOrganizationPatchNull),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganizationPatch {
    /// The new name of the organization.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
    /// The preferred new slug for the organization.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// ➕ Bencher Plus: The new license for the organization.
    /// Set to `null` to remove the current license.
    #[cfg(feature = "plus")]
    pub license: Option<bencher_valid::Jwt>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganizationPatchNull {
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
    #[cfg(feature = "plus")]
    pub license: (),
}

impl<'de> Deserialize<'de> for JsonUpdateOrganization {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const NAME_FIELD: &str = "name";
        const SLUG_FIELD: &str = "slug";
        #[cfg(feature = "plus")]
        const LICENSE_FIELD: &str = "license";
        const FIELDS: &[&str] = &[
            NAME_FIELD,
            SLUG_FIELD,
            #[cfg(feature = "plus")]
            LICENSE_FIELD,
        ];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Slug,
            #[cfg(feature = "plus")]
            License,
        }

        struct UpdateOrganizationVisitor;

        impl<'de> Visitor<'de> for UpdateOrganizationVisitor {
            type Value = JsonUpdateOrganization;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdateOrganization")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut name = None;
                let mut slug = None;
                #[cfg(feature = "plus")]
                let mut license = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field(NAME_FIELD));
                            }
                            name = Some(map.next_value()?);
                        },
                        Field::Slug => {
                            if slug.is_some() {
                                return Err(de::Error::duplicate_field(SLUG_FIELD));
                            }
                            slug = Some(map.next_value()?);
                        },
                        #[cfg(feature = "plus")]
                        Field::License => {
                            if license.is_some() {
                                return Err(de::Error::duplicate_field(LICENSE_FIELD));
                            }
                            license = Some(map.next_value()?);
                        },
                    }
                }

                #[cfg(not(feature = "plus"))]
                return Ok(Self::Value::Patch(JsonOrganizationPatch { name, slug }));
                #[cfg(feature = "plus")]
                return Ok(match license {
                    Some(Some(license)) => Self::Value::Patch(JsonOrganizationPatch {
                        name,
                        slug,
                        license: Some(license),
                    }),
                    Some(None) => Self::Value::Null(JsonOrganizationPatchNull {
                        name,
                        slug,
                        license: (),
                    }),
                    None => Self::Value::Patch(JsonOrganizationPatch {
                        name,
                        slug,
                        license: None,
                    }),
                });
            }
        }

        deserializer.deserialize_struct("JsonUpdateOrganization", FIELDS, UpdateOrganizationVisitor)
    }
}

#[cfg(feature = "plus")]
impl JsonUpdateOrganization {
    pub fn license(&self) -> Option<&bencher_valid::Jwt> {
        match self {
            Self::Patch(patch) => patch.license.as_ref(),
            Self::Null(_) => None,
        }
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Display)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPermission {
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
