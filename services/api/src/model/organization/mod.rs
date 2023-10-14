use std::str::FromStr;
use std::string::ToString;

use bencher_json::{
    organization::JsonUpdateOrganization, DateTime, JsonNewOrganization, JsonOrganization,
    NonEmpty, OrganizationUuid, ResourceId, Slug,
};
use bencher_rbac::Organization;
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::{DbConnection, Rbac},
    error::{forbidden_error, resource_not_found_err},
    model::user::{auth::AuthUser, InsertUser},
    schema::{self, organization as organization_table},
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
    },
};

pub mod member;
pub mod organization_role;

crate::util::typed_id::typed_id!(OrganizationId);

#[derive(Debug, Clone, Queryable, diesel::Identifiable)]
#[diesel(table_name = organization_table)]
pub struct QueryOrganization {
    pub id: OrganizationId,
    pub uuid: OrganizationUuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub subscription: Option<String>,
    pub license: Option<String>,
    pub created: DateTime,
    pub modified: DateTime,
}

#[cfg(feature = "plus")]
pub struct LicenseUsage {
    pub entitlements: u64,
    pub usage: u64,
}

impl QueryOrganization {
    fn_get!(organization, OrganizationId);
    fn_get_id!(organization, OrganizationId, OrganizationUuid);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid);

    pub fn from_resource_id(
        conn: &mut DbConnection,
        organization: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::organization::table
            .filter(resource_id(organization)?)
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Organization, organization))
    }

    #[cfg(feature = "plus")]
    pub fn get_subscription(&self) -> Result<Option<bencher_billing::SubscriptionId>, HttpError> {
        Ok(if let Some(subscription) = &self.subscription {
            Some(bencher_billing::SubscriptionId::from_str(subscription).map_err(|e| {
                crate::error::issue_error(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to parse subscription ID",
                    &format!("Failed to parse subscription ID ({subscription}) for organization ({self:?})"),
                    e,
                )
            })?)
        } else {
            None
        })
    }

    #[cfg(feature = "plus")]
    pub fn get_license(&self) -> Result<Option<bencher_json::Jwt>, HttpError> {
        Ok(if let Some(license) = &self.license {
            Some(bencher_json::Jwt::from_str(license).map_err(|e| {
                crate::error::issue_error(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to parse subscription license",
                    &format!("Failed to parse subscription license ({license}) for organization ({self:?})"),
                    e,
                )
            })?)
        } else {
            None
        })
    }

    #[cfg(feature = "plus")]
    pub fn check_license_usage(
        &self,
        conn: &mut DbConnection,
        licensor: &bencher_license::Licensor,
        license: &bencher_json::Jwt,
    ) -> Result<LicenseUsage, HttpError> {
        let token_data = licensor
            .validate_organization(license, self.uuid.into())
            .map_err(crate::error::payment_required_error)?;

        let start_time = token_data.claims.issued_at();
        let end_time = token_data.claims.expiration();

        let usage =
            super::project::metric::QueryMetric::usage(conn, self.id, start_time, end_time)?;
        let entitlements = licensor
            .validate_usage(&token_data.claims, usage)
            .map_err(crate::error::payment_required_error)?;

        Ok(LicenseUsage {
            entitlements,
            usage,
        })
    }

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

impl From<&QueryOrganization> for Organization {
    fn from(organization: &QueryOrganization) -> Self {
        Organization {
            id: organization.id.to_string(),
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
    pub fn from_json(conn: &mut DbConnection, organization: JsonNewOrganization) -> Self {
        let JsonNewOrganization { name, slug } = organization;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, organization, QueryOrganization);
        let timestamp = DateTime::now();
        Self {
            uuid: OrganizationUuid::new(),
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        }
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

fn_resource_id!(organization);

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = organization_table)]
pub struct UpdateOrganization {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub modified: DateTime,
}

impl From<JsonUpdateOrganization> for UpdateOrganization {
    fn from(update: JsonUpdateOrganization) -> Self {
        let JsonUpdateOrganization { name, slug } = update;
        Self {
            name,
            slug,
            modified: DateTime::now(),
        }
    }
}
