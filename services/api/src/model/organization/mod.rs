use std::str::FromStr;
use std::string::ToString;

#[cfg(feature = "plus")]
use bencher_billing::SubscriptionId;
#[cfg(feature = "plus")]
use bencher_json::Jwt;
use bencher_json::{
    organization::JsonUpdateOrganization, JsonNewOrganization, JsonOrganization, NonEmpty,
    ResourceId, Slug,
};
use bencher_rbac::Organization;
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};

use crate::{
    context::{DbConnection, Rbac},
    model::user::{auth::AuthUser, InsertUser},
    schema::{self, organization as organization_table},
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
        to_date_time,
    },
    ApiError,
};

pub mod member;
pub mod organization_role;

crate::util::typed_id::typed_id!(OrganizationId);
crate::util::typed_uuid::typed_uuid!(OrganizationUuid);

#[derive(diesel::Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: OrganizationUuid,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl InsertOrganization {
    pub fn from_json(conn: &mut DbConnection, organization: JsonNewOrganization) -> Self {
        let JsonNewOrganization { name, slug } = organization;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, organization, QueryOrganization);
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: OrganizationUuid::new(),
            name: name.into(),
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_user(insert_user: &InsertUser) -> Self {
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: OrganizationUuid::new(),
            name: insert_user.name.clone(),
            slug: insert_user.slug.clone(),
            created: timestamp,
            modified: timestamp,
        }
    }
}

fn_resource_id!(organization);

#[derive(Debug, Clone, Queryable, diesel::Identifiable)]
#[diesel(table_name = organization_table)]
pub struct QueryOrganization {
    pub id: OrganizationId,
    pub uuid: OrganizationUuid,
    pub name: String,
    pub slug: String,
    pub subscription: Option<String>,
    pub license: Option<String>,
    pub created: i64,
    pub modified: i64,
}

impl QueryOrganization {
    fn_get!(organization);
    fn_get_id!(organization, OrganizationId);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid);

    pub fn from_resource_id(
        conn: &mut DbConnection,
        organization: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::organization::table
            .filter(resource_id(organization)?)
            .first::<QueryOrganization>(conn)
            .map_err(ApiError::from)
    }

    #[cfg(feature = "plus")]
    pub fn get_subscription(
        conn: &mut DbConnection,
        resource_id: &ResourceId,
    ) -> Result<Option<SubscriptionId>, ApiError> {
        let organization = Self::from_resource_id(conn, resource_id)?;

        Ok(if let Some(subscription) = &organization.subscription {
            Some(SubscriptionId::from_str(subscription)?)
        } else {
            None
        })
    }

    #[cfg(feature = "plus")]
    pub fn get_license(
        conn: &mut DbConnection,
        resource_id: &ResourceId,
    ) -> Result<Option<(OrganizationUuid, Jwt)>, ApiError> {
        let organization = Self::from_resource_id(conn, resource_id)?;

        Ok(if let Some(license) = &organization.license {
            Some((organization.uuid, Jwt::from_str(license)?))
        } else {
            None
        })
    }

    pub fn is_allowed_resource_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, ApiError> {
        let query_organization = QueryOrganization::from_resource_id(conn, organization)?;

        rbac.is_allowed_organization(auth_user, permission, &query_organization)?;

        Ok(query_organization)
    }

    pub fn is_allowed_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization_id: OrganizationId,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, ApiError> {
        let query_organization = schema::organization::table
            .filter(schema::organization::id.eq(organization_id))
            .first(conn)
            .map_err(ApiError::from)?;

        rbac.is_allowed_organization(auth_user, permission, &query_organization)?;

        Ok(query_organization)
    }

    pub fn into_json(self) -> Result<JsonOrganization, ApiError> {
        let Self {
            uuid,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        Ok(JsonOrganization {
            uuid: uuid.into(),
            name: NonEmpty::from_str(&name).map_err(ApiError::from)?,
            slug: Slug::from_str(&slug).map_err(ApiError::from)?,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }
}

impl From<&QueryOrganization> for Organization {
    fn from(organization: &QueryOrganization) -> Self {
        Organization {
            id: organization.id.to_string(),
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = organization_table)]
pub struct UpdateOrganization {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub modified: i64,
}

impl From<JsonUpdateOrganization> for UpdateOrganization {
    fn from(update: JsonUpdateOrganization) -> Self {
        let JsonUpdateOrganization { name, slug } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            modified: Utc::now().timestamp(),
        }
    }
}
