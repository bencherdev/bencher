use std::string::ToString;

use bencher_json::{
    organization::{JsonOrganizationPatch, JsonOrganizationPatchNull, JsonUpdateOrganization},
    DateTime, JsonNewOrganization, JsonOrganization, Jwt, NonEmpty, OrganizationUuid, ResourceId,
    Slug,
};
use bencher_rbac::Organization;
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::{DbConnection, Rbac},
    error::forbidden_error,
    model::user::{auth::AuthUser, InsertUser},
    schema::{self, organization as organization_table},
    util::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_from_resource_id, fn_resource_id},
        slug::ok_slug,
    },
};

pub mod member;
pub mod organization_role;
#[cfg(feature = "plus")]
pub mod plan;

crate::util::typed_id::typed_id!(OrganizationId);

#[derive(Debug, Clone, Queryable, diesel::Identifiable)]
#[diesel(table_name = organization_table)]
pub struct QueryOrganization {
    pub id: OrganizationId,
    pub uuid: OrganizationUuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub license: Option<Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryOrganization {
    fn_resource_id!(organization);
    fn_from_resource_id!(organization, Organization, true);

    fn_get!(organization, OrganizationId);
    fn_get_id!(organization, OrganizationId, OrganizationUuid);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid);

    pub fn is_allowed_resource_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, HttpError> {
        let query_organization = Self::from_resource_id(conn, organization)?;
        rbac.is_allowed_organization(auth_user, permission, &query_organization)
            .map_err(forbidden_error)?;
        Ok(query_organization)
    }

    pub fn is_allowed_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization_id: OrganizationId,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, HttpError> {
        let query_organization = Self::get(conn, organization_id)?;
        rbac.is_allowed_organization(auth_user, permission, &query_organization)
            .map_err(forbidden_error)?;
        Ok(query_organization)
    }

    pub fn into_json(self) -> JsonOrganization {
        let Self {
            uuid,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        JsonOrganization {
            uuid,
            name,
            slug,
            created,
            modified,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: OrganizationUuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertOrganization {
    pub fn from_json(
        conn: &mut DbConnection,
        organization: JsonNewOrganization,
    ) -> Result<Self, HttpError> {
        let JsonNewOrganization { name, slug } = organization;
        let slug = ok_slug!(conn, &name, slug, organization, QueryOrganization)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: OrganizationUuid::new(),
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        })
    }

    pub fn from_user(insert_user: &InsertUser) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid: OrganizationUuid::new(),
            name: insert_user.name.clone().into(),
            slug: insert_user.slug.clone(),
            created: timestamp,
            modified: timestamp,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = organization_table)]
pub struct UpdateOrganization {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    #[cfg(feature = "plus")]
    pub license: Option<Option<Jwt>>,
    pub modified: DateTime,
}

impl From<JsonUpdateOrganization> for UpdateOrganization {
    fn from(update: JsonUpdateOrganization) -> Self {
        match update {
            JsonUpdateOrganization::Patch(patch) => {
                let JsonOrganizationPatch {
                    name,
                    slug,
                    #[cfg(feature = "plus")]
                    license,
                } = patch;
                Self {
                    name,
                    slug,
                    #[cfg(feature = "plus")]
                    license: license.map(Some),
                    modified: DateTime::now(),
                }
            },
            JsonUpdateOrganization::Null(patch_url) => {
                let JsonOrganizationPatchNull {
                    name,
                    slug,
                    #[cfg(feature = "plus")]
                        license: _,
                } = patch_url;
                Self {
                    name,
                    slug,
                    #[cfg(feature = "plus")]
                    license: Some(None),
                    modified: DateTime::now(),
                }
            },
        }
    }
}

impl From<OrganizationId> for Organization {
    fn from(org_id: OrganizationId) -> Self {
        Self {
            id: org_id.to_string(),
        }
    }
}

impl From<&QueryOrganization> for Organization {
    fn from(organization: &QueryOrganization) -> Self {
        Organization {
            id: organization.id.to_string(),
        }
    }
}
