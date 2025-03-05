use std::string::ToString;

use bencher_json::{
    organization::{
        member::OrganizationRole, JsonOrganizationPatch, JsonOrganizationPatchNull,
        JsonUpdateOrganization,
    },
    DateTime, JsonNewOrganization, JsonOrganization, Jwt, OrganizationUuid, ResourceId,
    ResourceName, Slug,
};
use bencher_rbac::{organization::Permission, Organization};
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;
use organization_role::{InsertOrganizationRole, QueryOrganizationRole};

use crate::{
    conn_lock,
    context::{DbConnection, Rbac},
    error::{
        forbidden_error, issue_error, resource_not_found_error, unauthorized_error, BencherResource,
    },
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    model::user::{auth::AuthUser, InsertUser},
    resource_conflict_err, resource_not_found_err,
    schema::{self, organization as organization_table},
    ApiContext,
};

use super::user::QueryUser;

pub mod member;
pub mod organization_role;
pub mod plan;

crate::macros::typed_id::typed_id!(OrganizationId);

#[derive(Debug, Clone, Queryable, diesel::Identifiable)]
#[diesel(table_name = organization_table)]
pub struct QueryOrganization {
    pub id: OrganizationId,
    pub uuid: OrganizationUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub license: Option<Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryOrganization {
    fn_eq_resource_id!(organization);
    fn_from_resource_id!(organization, Organization);

    fn_get!(organization, OrganizationId);
    fn_get_id!(organization, OrganizationId, OrganizationUuid);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid);

    pub async fn get_or_create_from_user(
        context: &ApiContext,
        auth_user: &AuthUser,
    ) -> Result<Self, HttpError> {
        let user_slug = &auth_user.user.slug;
        if let Ok(query_organization) =
            Self::from_resource_id(conn_lock!(context), &user_slug.clone().into())
        {
            query_organization
                .try_allowed(&context.rbac, auth_user, Permission::View)
                .map_err(|err| {
                    issue_error(
                        "User cannot view own organization",
                        &format!("User ({user_slug}) cannot view own organization."),
                        err,
                    )
                })?;
            return Ok(query_organization);
        }

        let insert_organization =
            InsertOrganization::new(auth_user.user.name.clone().into(), user_slug.clone());
        Self::create(context, auth_user, insert_organization).await
    }

    pub async fn get_or_create_from_project(
        context: &ApiContext,
        project_name: &ResourceName,
        project_slug: &Slug,
    ) -> Result<Self, HttpError> {
        if let Ok(query_organization) =
            Self::from_resource_id(conn_lock!(context), &project_slug.clone().into())
        {
            // If the project is part of an organization that is claimed,
            // then the project can not have anonymous reports.
            return if query_organization.is_claimed(conn_lock!(context))? {
                Err(unauthorized_error(format!(
                    "This project ({project_slug}) has already been claimed."
                )))
            } else {
                Ok(query_organization)
            };
        }

        let insert_organization =
            InsertOrganization::new(project_name.clone(), project_slug.clone());
        Self::create_inner(context, insert_organization).await
    }

    pub async fn create(
        context: &ApiContext,
        auth_user: &AuthUser,
        insert_organization: InsertOrganization,
    ) -> Result<Self, HttpError> {
        // Don't allow other users to create an organization with the same slug as another user.
        // This is needed to make on-the-fly projects for an authenticated user work.
        if insert_organization.slug != auth_user.user.slug
            && !auth_user.is_admin(&context.rbac)
            && QueryUser::from_resource_id(
                conn_lock!(context),
                &insert_organization.slug.clone().into(),
            )
            .is_ok()
        {
            return Err(forbidden_error(
                "You cannot create an organization with the same slug as a different user.",
            ));
        }

        let query_organization = Self::create_inner(context, insert_organization).await?;

        let timestamp = DateTime::now();
        // Connect the user to the organization as a `Leader`
        let insert_org_role = InsertOrganizationRole {
            user_id: auth_user.id,
            organization_id: query_organization.id,
            role: OrganizationRole::Leader,
            created: timestamp,
            modified: timestamp,
        };
        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(OrganizationRole, insert_org_role))?;

        Ok(query_organization)
    }

    async fn create_inner(
        context: &ApiContext,
        insert_organization: InsertOrganization,
    ) -> Result<Self, HttpError> {
        diesel::insert_into(schema::organization::table)
            .values(&insert_organization)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Organization, insert_organization))?;
        schema::organization::table
            .filter(schema::organization::uuid.eq(&insert_organization.uuid))
            .first::<QueryOrganization>(conn_lock!(context))
            .map_err(resource_not_found_err!(Organization, insert_organization))
    }

    pub fn is_allowed_resource_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization: &ResourceId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        // Do not leak information about organizations.
        // Always return the same error.
        Self::is_allowed_resource_id_inner(conn, rbac, organization, auth_user, permission).map_err(
            |_e| resource_not_found_error(BencherResource::Organization, organization, permission),
        )
    }

    fn is_allowed_resource_id_inner(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization: &ResourceId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        let query_organization = Self::from_resource_id(conn, organization)?;
        query_organization.try_allowed(rbac, auth_user, permission)?;
        Ok(query_organization)
    }

    pub fn is_allowed_id(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization_id: OrganizationId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        // Do not leak information about organizations.
        // Always return the same error.
        Self::is_allowed_id_inner(conn, rbac, organization_id, auth_user, permission).map_err(
            |_e| {
                resource_not_found_error(BencherResource::Organization, organization_id, permission)
            },
        )
    }

    fn is_allowed_id_inner(
        conn: &mut DbConnection,
        rbac: &Rbac,
        organization_id: OrganizationId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        let query_organization = Self::get(conn, organization_id)?;
        query_organization.try_allowed(rbac, auth_user, permission)?;
        Ok(query_organization)
    }

    fn try_allowed(
        &self,
        rbac: &Rbac,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<(), HttpError> {
        rbac.is_allowed_organization(auth_user, permission, self)
            .map_err(forbidden_error)
    }

    pub fn is_claimed(&self, conn: &mut DbConnection) -> Result<bool, HttpError> {
        let total_members = QueryOrganizationRole::count(conn, self.id)?;
        // If the organization that has zero members, then it is unclaimed.
        Ok(total_members > 0)
    }

    pub fn claimed_at(&self, conn: &mut DbConnection) -> Result<DateTime, HttpError> {
        QueryOrganizationRole::claimed_at(conn, self.id)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> JsonOrganization {
        let claimed = self.claimed_at(conn).ok();
        let Self {
            uuid,
            name,
            slug,
            #[cfg(feature = "plus")]
            license,
            created,
            modified,
            ..
        } = self;
        JsonOrganization {
            uuid,
            name,
            slug,
            #[cfg(feature = "plus")]
            license,
            created,
            modified,
            claimed,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = organization_table)]
pub struct InsertOrganization {
    pub uuid: OrganizationUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertOrganization {
    fn new(name: ResourceName, slug: Slug) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid: OrganizationUuid::new(),
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_json(
        conn: &mut DbConnection,
        organization: JsonNewOrganization,
    ) -> Result<Self, HttpError> {
        let JsonNewOrganization { name, slug } = organization;
        let slug = ok_slug!(conn, &name, slug, organization, QueryOrganization)?;
        Ok(Self::new(name, slug))
    }

    pub fn from_user(insert_user: &InsertUser) -> Self {
        let InsertUser { name, slug, .. } = insert_user;
        Self::new(name.clone().into(), slug.clone())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = organization_table)]
pub struct UpdateOrganization {
    pub name: Option<ResourceName>,
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
                        license: (),
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
