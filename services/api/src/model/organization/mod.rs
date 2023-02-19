use std::str::FromStr;
use std::string::ToString;

use bencher_json::{JsonNewOrganization, JsonOrganization, NonEmpty, ResourceId, Slug};
use bencher_rbac::Organization;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use crate::{
    context::ApiContext,
    error::api_error,
    model::user::{auth::AuthUser, InsertUser},
    schema::{self, organization as organization_table},
    util::{query::fn_get_id, resource_id::fn_resource_id, slug::unwrap_slug},
    ApiError,
};

pub mod member;
pub mod organization_role;

#[derive(Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: String,
    pub name: String,
    pub slug: String,
}

impl InsertOrganization {
    pub fn from_json(conn: &mut SqliteConnection, organization: JsonNewOrganization) -> Self {
        let JsonNewOrganization { name, slug } = organization;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, organization, QueryOrganization);
        Self {
            uuid: Uuid::new_v4().to_string(),
            name: name.into(),
            slug,
        }
    }

    pub fn from_user(insert_user: &InsertUser) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            name: insert_user.name.clone(),
            slug: insert_user.slug.clone(),
        }
    }
}

fn_resource_id!(organization);

#[derive(Debug, Clone, Queryable)]
pub struct QueryOrganization {
    pub id: i32,
    pub uuid: String,
    pub name: String,
    pub slug: String,
    pub subscription: Option<String>,
    pub license: Option<String>,
}

impl QueryOrganization {
    fn_get_id!(organization);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::organization::table
            .filter(schema::organization::id.eq(id))
            .select(schema::organization::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        organization: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::organization::table
            .filter(resource_id(organization)?)
            .first::<QueryOrganization>(conn)
            .map_err(api_error!())
    }

    pub fn is_allowed_resource_id(
        api_context: &mut ApiContext,
        organization: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, ApiError> {
        let query_organization = QueryOrganization::from_resource_id(
            &mut api_context.database.connection,
            organization,
        )?;

        api_context
            .rbac
            .is_allowed_organization(auth_user, permission, &query_organization)?;

        Ok(query_organization)
    }

    pub fn is_allowed_id(
        api_context: &mut ApiContext,
        organization_id: i32,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
    ) -> Result<Self, ApiError> {
        let query_organization = schema::organization::table
            .filter(schema::organization::id.eq(organization_id))
            .first(&mut api_context.database.connection)
            .map_err(api_error!())?;

        api_context
            .rbac
            .is_allowed_organization(auth_user, permission, &query_organization)?;

        Ok(query_organization)
    }

    pub fn into_json(self) -> Result<JsonOrganization, ApiError> {
        let Self {
            uuid, name, slug, ..
        } = self;
        Ok(JsonOrganization {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            name: NonEmpty::from_str(&name).map_err(api_error!())?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
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
