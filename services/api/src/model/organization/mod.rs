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
use derive_more::Display;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::{DbConnection, Rbac},
    error::api_error,
    model::user::{auth::AuthUser, InsertUser},
    schema::{self, organization as organization_table},
    util::{
        query::{fn_get, fn_get_id},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
        to_date_time,
    },
    ApiError,
};

pub mod member;
pub mod organization_role;

// https://github.com/diesel-rs/diesel/blob/master/diesel_tests/tests/custom_types.rs
#[derive(Debug, Clone, Copy, Display, PartialEq, Eq, FromSqlRow, AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub struct OrganizationId(i32);

impl From<OrganizationId> for i32 {
    fn from(id: OrganizationId) -> Self {
        id.0
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for OrganizationId
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for OrganizationId
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(i32::from_sql(bytes)?))
    }
}

#[derive(Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: String,
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
            uuid: Uuid::new_v4().to_string(),
            name: name.into(),
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_user(insert_user: &InsertUser) -> Self {
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: Uuid::new_v4().to_string(),
            name: insert_user.name.clone(),
            slug: insert_user.slug.clone(),
            created: timestamp,
            modified: timestamp,
        }
    }
}

fn_resource_id!(organization);

#[derive(Debug, Clone, Queryable)]
pub struct QueryOrganization {
    pub id: OrganizationId,
    pub uuid: String,
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

    pub fn get_uuid(conn: &mut DbConnection, id: OrganizationId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::organization::table
            .filter(schema::organization::id.eq(id))
            .select(schema::organization::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        organization: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::organization::table
            .filter(resource_id(organization)?)
            .first::<QueryOrganization>(conn)
            .map_err(api_error!())
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
    ) -> Result<Option<(Uuid, Jwt)>, ApiError> {
        let organization = Self::from_resource_id(conn, resource_id)?;

        Ok(if let Some(license) = &organization.license {
            Some((Uuid::from_str(&organization.uuid)?, Jwt::from_str(license)?))
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
            .map_err(api_error!())?;

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
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            name: NonEmpty::from_str(&name).map_err(api_error!())?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
            created: to_date_time(created).map_err(api_error!())?,
            modified: to_date_time(modified).map_err(api_error!())?,
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

#[derive(Debug, Clone, AsChangeset)]
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
