use std::string::ToString as _;

use bencher_json::{
    DateTime, IntoResourceId as _, JsonNewOrganization, JsonOrganization, Jwt,
    OrganizationResourceId, OrganizationSlug, OrganizationUuid, ProjectSlug, ResourceName,
    organization::{
        JsonOrganizationPatch, JsonOrganizationPatchNull, JsonUpdateOrganization,
        member::OrganizationRole,
    },
};
use bencher_rbac::{Organization, organization::Permission};
use diesel::{ExpressionMethods as _, QueryDsl as _, Queryable, RunQueryDsl as _};
use dropshot::HttpError;
use organization_role::{InsertOrganizationRole, QueryOrganizationRole};

use crate::{
    ApiContext, CLAIM_TOKEN_TTL, conn_lock,
    context::{DbConnection, Rbac},
    error::{
        BencherResource, forbidden_error, issue_error, resource_not_found_error, unauthorized_error,
    },
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    model::user::auth::AuthUser,
    resource_conflict_err, resource_not_found_err,
    schema::{self, organization as organization_table},
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
    pub slug: OrganizationSlug,
    pub license: Option<Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryOrganization {
    fn_eq_resource_id!(organization, OrganizationResourceId);
    fn_from_resource_id!(organization, Organization, OrganizationResourceId);

    pub fn from_uuid(conn: &mut DbConnection, uuid: OrganizationUuid) -> Result<Self, HttpError> {
        schema::organization::table
            .filter(schema::organization::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Organization, uuid))
    }

    fn_get!(organization, OrganizationId);
    fn_get_id!(organization, OrganizationId, OrganizationUuid);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid);

    pub async fn get_or_create_from_user(
        context: &ApiContext,
        auth_user: &AuthUser,
    ) -> Result<Self, HttpError> {
        // The user's organization should be created with the user's UUID.
        let user_uuid = auth_user.user.uuid;
        if let Ok(query_organization) = Self::from_uuid(conn_lock!(context), user_uuid.into()) {
            query_organization
                .try_allowed(&context.rbac, auth_user, Permission::View)
                .map_err(|err| {
                    issue_error(
                        "User cannot view own organization",
                        &format!("User ({user_uuid}) cannot view own organization."),
                        err,
                    )
                })?;
            return Ok(query_organization);
        }

        let insert_organization =
            InsertOrganization::from_user(conn_lock!(context), &auth_user.user);
        Self::create(context, auth_user, insert_organization).await
    }

    pub async fn get_or_create_from_project(
        context: &ApiContext,
        project_name: &ResourceName,
        project_slug: &ProjectSlug,
    ) -> Result<Self, HttpError> {
        // The project organization should be created with the project's slug.
        if let Ok(query_organization) = Self::from_resource_id(
            conn_lock!(context),
            &OrganizationSlug::from(project_slug.clone()).into_resource_id(),
        ) {
            // If the project is part of an organization that is claimed,
            // then the project can not have anonymous reports.
            return if query_organization.is_claimed(conn_lock!(context))? {
                Err(unauthorized_error(format!(
                    "This project ({project_slug}) has already been claimed. Provide a valid API token (`--token`) to authenticate."
                )))
            } else {
                Ok(query_organization)
            };
        }

        let insert_organization =
            InsertOrganization::new(project_name.clone(), project_slug.clone().into());
        Self::create_inner(context, insert_organization).await
    }

    pub async fn create(
        context: &ApiContext,
        auth_user: &AuthUser,
        insert_organization: InsertOrganization,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        InsertOrganization::rate_limit(context, auth_user).await?;
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
        organization: &OrganizationResourceId,
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
        organization: &OrganizationResourceId,
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

    pub async fn claim(
        &self,
        context: &ApiContext,
        query_user: &QueryUser,
    ) -> Result<(), HttpError> {
        if self.is_claimed(conn_lock!(context))? {
            return Err(unauthorized_error(format!(
                "This organization ({}) has already been claimed.",
                self.uuid
            )));
        }

        // Create an invite token to claim the organization
        let invite = context
            .token_key
            .new_invite(
                query_user.email.clone(),
                CLAIM_TOKEN_TTL,
                self.uuid,
                OrganizationRole::Leader,
            )
            .map_err(|e| {
                issue_error(
                    "Failed to create new claim token",
                    "Failed to create new claim token.",
                    e,
                )
            })?;

        query_user.accept_invite(conn_lock!(context), &context.token_key, &invite)
    }

    #[cfg(feature = "plus")]
    pub async fn daily_usage(&self, context: &ApiContext) -> Result<u32, HttpError> {
        use crate::model::project::metric::QueryMetric;

        let (start_time, end_time) = context.rate_limiting.window();
        QueryMetric::usage(conn_lock!(context), self.id, start_time, end_time)
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
    pub slug: OrganizationSlug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertOrganization {
    #[cfg(feature = "plus")]
    pub async fn rate_limit(context: &ApiContext, query_user: &QueryUser) -> Result<(), HttpError> {
        use crate::context::RateLimitingError;

        let resource = BencherResource::Organization;
        let (start_time, end_time) = context.rate_limiting.window();
        let creation_count: u32 = schema::organization::table
                .inner_join(schema::organization_role::table)
                .filter(schema::organization_role::user_id.eq(query_user.id))
                .filter(schema::organization::created.ge(start_time))
                .filter(schema::organization::created.le(end_time))
                .count()
                .get_result::<i64>(conn_lock!(context))
                .map_err(resource_not_found_err!(Organization, (query_user, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for user ({uuid}) between {start_time} and {end_time}.", uuid = query_user.uuid),
                    e
                    )}
                )?;

        let rate_limit = context.rate_limiting.user_limit;
        if creation_count >= rate_limit {
            Err(crate::error::too_many_requests(RateLimitingError::User {
                user: query_user.clone(),
                resource,
                rate_limit,
            }))
        } else {
            Ok(())
        }
    }

    fn new(name: ResourceName, slug: OrganizationSlug) -> Self {
        Self::new_inner(OrganizationUuid::new(), name, slug)
    }

    fn new_inner(uuid: OrganizationUuid, name: ResourceName, slug: OrganizationSlug) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_json(conn: &mut DbConnection, organization: JsonNewOrganization) -> Self {
        let JsonNewOrganization { name, slug } = organization;
        let slug = ok_slug!(
            conn,
            &name,
            slug.map(Into::into),
            organization,
            QueryOrganization
        );
        Self::new(name, slug.into())
    }

    pub fn from_user(conn: &mut DbConnection, query_user: &QueryUser) -> Self {
        let name = query_user.name.clone();
        // Because users are now allowed to create arbitrary organizations,
        // we need to check if the slug is already in use.
        let slug = ok_slug!(
            conn,
            &name,
            Some(query_user.slug.clone()),
            organization,
            QueryOrganization
        );
        // The user's organization should be created with the user's UUID.
        Self::new_inner(query_user.uuid.into(), name.into(), slug.into())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = organization_table)]
pub struct UpdateOrganization {
    pub name: Option<ResourceName>,
    pub slug: Option<OrganizationSlug>,
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
