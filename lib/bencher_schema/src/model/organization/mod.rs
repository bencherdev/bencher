use std::string::ToString as _;

#[cfg(feature = "plus")]
use bencher_json::JsonSsos;
use bencher_json::{
    DateTime, JsonNewOrganization, JsonOrganization, Jwt, OrganizationResourceId, OrganizationSlug,
    OrganizationUuid, ProjectSlug, ResourceName,
    organization::{
        JsonOrganizationPatch, JsonOrganizationPatchNull, JsonUpdateOrganization,
        member::OrganizationRole,
    },
};
use bencher_rbac::{Organization, organization::Permission};
#[cfg(feature = "plus")]
use diesel::BelongingToDsl as _;
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, Queryable, RunQueryDsl as _,
};
use dropshot::HttpError;
use organization_role::{InsertOrganizationRole, QueryOrganizationRole};
#[cfg(feature = "plus")]
use sso::QuerySso;

use crate::{
    ApiContext, CLAIM_TOKEN_TTL, auth_conn,
    context::{DbConnection, Rbac},
    error::{
        BencherResource, InsertRefusal, forbidden_error, issue_error, resource_conflict_error,
        resource_not_found_error, unauthorized_error,
    },
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
        sql::last_insert_rowid,
    },
    model::user::{UserId, auth::AuthUser},
    resource_conflict_err, resource_not_found_err,
    schema::{self, organization as organization_table},
    write_conn, write_transaction,
};
#[cfg(feature = "plus")]
use crate::{actor_conn, model::user::actor::ApiActor};

use super::user::QueryUser;

pub mod member;
pub mod organization_role;
pub mod plan;
pub mod sso;

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
    pub deleted: Option<DateTime>,
}

impl QueryOrganization {
    fn_eq_resource_id!(organization, OrganizationResourceId);
    fn_from_resource_id!(
        organization,
        Organization,
        OrganizationResourceId,
        not_deleted
    );

    fn_get!(organization, OrganizationId, not_deleted);
    fn_get_id!(organization, OrganizationId, OrganizationUuid, not_deleted);
    fn_get_uuid!(organization, OrganizationId, OrganizationUuid, not_deleted);
    fn_from_uuid!(organization, OrganizationUuid, Organization, not_deleted);

    pub async fn get_or_create_from_user(
        context: &ApiContext,
        auth_user: &AuthUser,
    ) -> Result<Self, HttpError> {
        // The user's organization should be created with the user's UUID.
        let user_uuid = auth_user.user.uuid;
        let organization_uuid = OrganizationUuid::from(user_uuid);
        let found = Self::from_uuid(auth_conn!(context), organization_uuid);
        if let Ok(query_organization) = found
            && query_organization
                .try_allowed(&context.rbac, auth_user, Permission::View)
                .is_ok()
        {
            return Ok(query_organization);
        }

        // The lookup, the creation limit, and the insert share one write transaction,
        // so only the request that creates the organization counts toward the limit.
        let user_id = auth_user.id;
        let inserted = write_transaction!(context, |conn| {
            let existing = schema::organization::table
                .filter(schema::organization::uuid.eq(organization_uuid))
                .filter(schema::organization::deleted.is_null())
                .select(schema::organization::id)
                .first::<OrganizationId>(conn)
                .optional()?;
            if existing.is_some() {
                return Ok(None);
            }
            #[cfg(feature = "plus")]
            context
                .rate_limiting
                .create_organization(user_uuid)
                .map_err(InsertRefusal::Refused)?;
            let insert_organization = InsertOrganization::from_user(conn, &auth_user.user);
            let id =
                Self::insert_with_leader(conn, &insert_organization, user_id).map_err(|e| {
                    InsertRefusal::Refused(resource_conflict_error(
                        BencherResource::Organization,
                        &insert_organization,
                        e,
                    ))
                })?;
            Ok::<_, InsertRefusal>(Some(insert_organization.into_query(id)))
        })
        .map_err(|refusal| {
            refusal.into_http_error(BencherResource::Organization, organization_uuid)
        })?;

        if let Some(query_organization) = inserted {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OrganizationCreate);
            return Ok(query_organization);
        }
        // Another request created the organization, along with this user's role in it,
        // after this user was loaded. A user who left their organization conflicts with it.
        let auth_user = auth_user.reload(auth_conn!(context))?;
        let query_organization = Self::from_uuid(auth_conn!(context), organization_uuid)?;
        query_organization
            .try_allowed(&context.rbac, &auth_user, Permission::View)
            .map_err(|e| {
                resource_conflict_error(BencherResource::Organization, organization_uuid, e)
            })?;
        Ok(query_organization)
    }

    /// Find the organization of an on-the-fly project by the project slug, or insert it.
    /// This runs in the write transaction that inserts the project,
    /// so the organization is only kept along with its project.
    pub(crate) fn get_or_insert_for_project(
        conn: &mut DbConnection,
        project_name: &ResourceName,
        project_slug: &ProjectSlug,
    ) -> diesel::QueryResult<GetOrInsertOrganization> {
        let slug = OrganizationSlug::from(project_slug.clone());
        let existing = schema::organization::table
            .filter(schema::organization::slug.eq(&slug))
            .filter(schema::organization::deleted.is_null())
            .first::<Self>(conn)
            .optional()?;
        if let Some(existing) = existing {
            return Ok(
                if QueryOrganizationRole::count_query(conn, existing.id)? == 0 {
                    GetOrInsertOrganization::Unclaimed(existing)
                } else {
                    GetOrInsertOrganization::Claimed
                },
            );
        }

        let insert_organization = InsertOrganization::new(project_name.clone(), slug);
        let id = Self::insert(conn, &insert_organization)?;
        Ok(GetOrInsertOrganization::Inserted(
            insert_organization.into_query(id),
        ))
    }

    pub async fn create(
        context: &ApiContext,
        auth_user: &AuthUser,
        insert_organization: InsertOrganization,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        context
            .rate_limiting
            .create_organization(auth_user.user.uuid)?;

        let user_id = auth_user.id;
        let query_organization = write_transaction!(context, |conn| {
            Self::insert_with_leader(conn, &insert_organization, user_id)
        })
        .map_err(resource_conflict_err!(Organization, &insert_organization))
        .map(|id| insert_organization.into_query(id))?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OrganizationCreate);

        Ok(query_organization)
    }

    /// Insert the organization, and connect the user to it as a `Leader`.
    fn insert_with_leader(
        conn: &mut DbConnection,
        insert_organization: &InsertOrganization,
        user_id: UserId,
    ) -> diesel::QueryResult<OrganizationId> {
        let id = Self::insert(conn, insert_organization)?;
        let timestamp = DateTime::now();
        let insert_org_role = InsertOrganizationRole {
            user_id,
            organization_id: id,
            role: OrganizationRole::Leader,
            created: timestamp,
            modified: timestamp,
        };
        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)?;
        Ok(id)
    }

    fn insert(
        conn: &mut DbConnection,
        insert_organization: &InsertOrganization,
    ) -> diesel::QueryResult<OrganizationId> {
        diesel::insert_into(schema::organization::table)
            .values(insert_organization)
            .execute(conn)?;
        diesel::select(last_insert_rowid()).get_result(conn)
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
        Ok(QueryOrganizationRole::count(conn, self.id)? > 0)
    }

    pub fn claimed_at(&self, conn: &mut DbConnection) -> Result<DateTime, HttpError> {
        QueryOrganizationRole::claimed_at(conn, self.id)
    }

    pub async fn claim(
        &self,
        log: &slog::Logger,
        context: &ApiContext,
        query_user: &QueryUser,
    ) -> Result<(), HttpError> {
        if self.is_claimed(auth_conn!(context))? {
            return Err(self.already_claimed_error());
        }
        if let Claim::Taken = self.try_claim(log, context, query_user).await? {
            return Err(self.already_claimed_error());
        }
        Ok(())
    }

    /// Claim the organization for the user, unless another user claimed it first.
    /// The check for a claim and the user's `Leader` role share one write transaction,
    /// so only one request claims the organization.
    pub async fn try_claim(
        &self,
        log: &slog::Logger,
        context: &ApiContext,
        query_user: &QueryUser,
    ) -> Result<Claim, HttpError> {
        let organization_id = self.id;
        let user_id = query_user.id;
        let (claim, inserted) = write_transaction!(context, |conn| {
            if QueryOrganizationRole::count_query(conn, organization_id)? == 0 {
                let timestamp = DateTime::now();
                diesel::insert_into(schema::organization_role::table)
                    .values(&InsertOrganizationRole {
                        user_id,
                        organization_id,
                        role: OrganizationRole::Leader,
                        created: timestamp,
                        modified: timestamp,
                    })
                    .execute(conn)?;
                return Ok((Claim::Leader, true));
            }
            let role = schema::organization_role::table
                .filter(schema::organization_role::organization_id.eq(organization_id))
                .filter(schema::organization_role::user_id.eq(user_id))
                .select(schema::organization_role::role)
                .first::<OrganizationRole>(conn)
                .optional()?;
            let claim = if role == Some(OrganizationRole::Leader) {
                Claim::Leader
            } else {
                Claim::Taken
            };
            Ok::<_, diesel::result::Error>((claim, false))
        })
        .map_err(resource_conflict_err!(
            OrganizationRole,
            (self.uuid, query_user.uuid)
        ))?;

        if inserted {
            // Upgrade any pending Unclaimed jobs for this organization to Free priority
            #[cfg(feature = "plus")]
            self.upgrade_pending_jobs(log, context).await?;

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserClaim);
        }
        #[cfg(not(feature = "plus"))]
        let _ = log;

        Ok(claim)
    }

    fn already_claimed_error(&self) -> HttpError {
        unauthorized_error(format!(
            "This organization ({}) has already been claimed.",
            self.uuid
        ))
    }

    /// When an organization is claimed, upgrade all of its pending jobs
    /// from `Unclaimed` priority to `Free` priority.
    #[cfg(feature = "plus")]
    async fn upgrade_pending_jobs(
        &self,
        log: &slog::Logger,
        context: &ApiContext,
    ) -> Result<(), HttpError> {
        use bencher_json::{JobStatus, Priority};

        let count = diesel::update(
            schema::job::table
                .filter(schema::job::organization_id.eq(self.id))
                .filter(schema::job::status.eq(JobStatus::Pending))
                .filter(schema::job::priority.eq(Priority::Unclaimed)),
        )
        .set((
            schema::job::priority.eq(Priority::Free),
            schema::job::modified.eq(DateTime::now()),
        ))
        .execute(write_conn!(context))
        .map_err(|e| {
            issue_error(
                "Failed to upgrade pending job priorities",
                &format!(
                    "Failed to upgrade pending Unclaimed jobs to Free for organization ({}).",
                    self.uuid,
                ),
                e,
            )
        })?;

        slog::info!(
            log,
            "Upgraded {count} pending job(s) from Unclaimed to Free priority";
            "organization" => %self.uuid,
            "count" => count,
        );

        Ok(())
    }

    pub async fn join(
        context: &ApiContext,
        organization_uuid: OrganizationUuid,
        query_user: &QueryUser,
    ) -> Result<(), HttpError> {
        // Create an invite token to join the organization
        let invite = context
            .token_key
            .new_invite(
                query_user.email.clone(),
                CLAIM_TOKEN_TTL,
                organization_uuid,
                OrganizationRole::Leader,
            )
            .map_err(|e| {
                issue_error(
                    "Failed to create new claim token",
                    "Failed to create new claim token.",
                    e,
                )
            })?;

        // Accept the invite to join the organization
        query_user
            .accept_invite(context, &context.token_key, &invite)
            .await?;

        Ok(())
    }

    #[cfg(feature = "plus")]
    pub async fn window_usage(
        &self,
        context: &ApiContext,
        api_actor: &ApiActor,
    ) -> Result<u32, HttpError> {
        use crate::model::project::metric::QueryMetric;

        let (start_time, end_time) = context.rate_limiting.window();
        QueryMetric::rollup_usage(
            actor_conn!(context, api_actor),
            self.id,
            start_time,
            end_time,
        )
    }

    #[cfg(feature = "plus")]
    fn sso(&self, conn: &mut DbConnection) -> Result<Option<JsonSsos>, HttpError> {
        let query_sso = QuerySso::belonging_to(self)
            .order(schema::sso::domain.asc())
            .load::<QuerySso>(conn)
            .map_err(resource_not_found_err!(Sso, self.uuid))?;

        Ok(if query_sso.is_empty() {
            None
        } else {
            Some(query_sso.into_iter().map(QuerySso::into_json).collect())
        })
    }

    /// Soft-delete this organization and all its non-deleted child projects.
    /// Sets the `deleted` timestamp and replaces name/slug with valid
    /// `Deleted {uuid}` / `deleted-{uuid}` sentinels to free UNIQUE constraints.
    ///
    /// Child projects are soft-deleted first (including search index cleanup)
    /// so the org rename can be retried if any project rename fails.
    pub async fn soft_delete(
        &self,
        context: &ApiContext,
        log: &slog::Logger,
        now: DateTime,
    ) -> Result<(), HttpError> {
        use super::project::QueryProject;

        slog::info!(log, "Soft-deleting organization: {}", self.uuid);

        // Soft-delete all non-deleted projects under this org first
        let projects: Vec<QueryProject> = schema::project::table
            .filter(schema::project::organization_id.eq(self.id))
            .filter(schema::project::deleted.is_null())
            .load(auth_conn!(context))
            .map_err(resource_not_found_err!(Organization, self))?;
        for project in &projects {
            project.soft_delete(context, log, now).await?;
        }

        // Soft-delete the org (after all projects are successfully soft-deleted)
        let deleted_name: ResourceName = format!("Deleted {}", self.uuid).parse().map_err(|e| {
            issue_error(
                "Failed to create deleted organization name",
                &format!("Organization: {}", self.uuid),
                e,
            )
        })?;
        let deleted_slug: OrganizationSlug =
            format!("deleted-{}", self.uuid).parse().map_err(|e| {
                issue_error(
                    "Failed to create deleted organization slug",
                    &format!("Organization: {}", self.uuid),
                    e,
                )
            })?;
        diesel::update(schema::organization::table.filter(schema::organization::id.eq(self.id)))
            .set((
                schema::organization::deleted.eq(now),
                schema::organization::modified.eq(now),
                schema::organization::name.eq(deleted_name),
                schema::organization::slug.eq(deleted_slug),
            ))
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Organization, self))?;

        Ok(())
    }

    #[cfg(feature = "plus")]
    pub fn oci_bandwidth_priority(
        &self,
        conn: &mut DbConnection,
        licensor: &bencher_license::Licensor,
    ) -> Result<bencher_json::Priority, HttpError> {
        use bencher_json::Priority;
        use diesel::{BelongingToDsl as _, RunQueryDsl as _};

        if !self.is_claimed(conn)? {
            return Ok(Priority::Unclaimed);
        }

        // Check for a plan row (metered or licensed)
        if let Ok(query_plan) = plan::QueryPlan::belonging_to(self).first::<plan::QueryPlan>(conn)
            && (query_plan.metered_plan.is_some() || query_plan.licensed_plan.is_some())
        {
            return Ok(Priority::Plus);
        }

        // Check organization-level license
        if let Some(license) = &self.license
            && licensor.validate_organization(license, self.uuid).is_ok()
        {
            return Ok(Priority::Plus);
        }

        Ok(Priority::Free)
    }

    pub fn into_json_full(self, conn: &mut DbConnection) -> Result<JsonOrganization, HttpError> {
        #[cfg(feature = "plus")]
        let sso = self.sso(conn)?;
        Ok(self.into_json_inner(
            conn,
            #[cfg(feature = "plus")]
            sso,
        ))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> JsonOrganization {
        self.into_json_inner(
            conn,
            #[cfg(feature = "plus")]
            None,
        )
    }

    fn into_json_inner(
        self,
        conn: &mut DbConnection,
        #[cfg(feature = "plus")] sso: Option<JsonSsos>,
    ) -> JsonOrganization {
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
            #[cfg(feature = "plus")]
            sso: sso.map(Into::into),
            created,
            modified,
            claimed,
        }
    }
}

/// How a claim of an organization ended.
#[derive(Debug, Clone, Copy)]
pub enum Claim {
    /// The user is the `Leader` of the organization: this request claimed it,
    /// or a concurrent request by the same user claimed it first.
    Leader,
    /// Another user claimed the organization first.
    Taken,
}

pub(crate) enum GetOrInsertOrganization {
    Unclaimed(QueryOrganization),
    Inserted(QueryOrganization),
    Claimed,
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

    pub fn into_query(self, id: OrganizationId) -> QueryOrganization {
        let Self {
            uuid,
            name,
            slug,
            created,
            modified,
        } = self;
        QueryOrganization {
            id,
            uuid,
            name,
            slug,
            license: None,
            created,
            modified,
            deleted: None,
        }
    }

    pub fn from_json(conn: &mut DbConnection, organization: JsonNewOrganization) -> Self {
        let JsonNewOrganization { name, slug } = organization;
        let slug = ok_slug!(conn, &name, slug, organization, QueryOrganization);
        Self::new(name, slug)
    }

    pub fn from_user(conn: &mut DbConnection, query_user: &QueryUser) -> Self {
        let name = query_user.name.clone();
        // Because users are now allowed to create arbitrary organizations,
        // we need to check if the slug is already in use.
        let slug = ok_slug!(
            conn,
            &name,
            Some(query_user.slug.clone().into()),
            organization,
            QueryOrganization
        );
        // The user's organization should be created with the user's UUID.
        Self::new_inner(query_user.uuid.into(), name.into(), slug)
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
        Self {
            id: organization.id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use super::OrganizationId;
    use crate::{macros::sql::last_insert_rowid, schema, test_util::setup_test_db};

    #[test]
    fn last_insert_rowid_returns_organization_id() {
        let mut conn = setup_test_db();
        let uuid = "00000000-0000-0000-0000-000000000010";

        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::organization::table)
                    .values((
                        schema::organization::uuid.eq(uuid),
                        schema::organization::name.eq("Org 1"),
                        schema::organization::slug.eq("org-1"),
                        schema::organization::created.eq(DateTime::TEST),
                        schema::organization::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid =
                    diesel::select(last_insert_rowid()).get_result::<OrganizationId>(conn)?;
                let select_id: OrganizationId = schema::organization::table
                    .filter(schema::organization::uuid.eq(uuid))
                    .select(schema::organization::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_organization() {
        let mut conn = setup_test_db();

        // Insert first
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::organization::name.eq("Org 1"),
                schema::organization::slug.eq("org-1"),
                schema::organization::created.eq(DateTime::TEST),
                schema::organization::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first organization");

        // Insert second + verify
        let second_uuid = "00000000-0000-0000-0000-000000000011";
        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::organization::table)
                    .values((
                        schema::organization::uuid.eq(second_uuid),
                        schema::organization::name.eq("Org 2"),
                        schema::organization::slug.eq("org-2"),
                        schema::organization::created.eq(DateTime::TEST),
                        schema::organization::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid =
                    diesel::select(last_insert_rowid()).get_result::<OrganizationId>(conn)?;
                let select_id: OrganizationId = schema::organization::table
                    .filter(schema::organization::uuid.eq(second_uuid))
                    .select(schema::organization::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);

        let first_id: OrganizationId = schema::organization::table
            .filter(schema::organization::uuid.eq("00000000-0000-0000-0000-000000000010"))
            .select(schema::organization::id)
            .first(&mut conn)
            .expect("Failed to get first organization id");
        assert_ne!(rowid, first_id);
    }
}
