use std::{string::ToString as _, sync::LazyLock};

use bencher_json::{
    DateTime, JsonNewProject, JsonProject, ProjectResourceId, ProjectSlug, ProjectUuid,
    ResourceName, Url,
    project::{JsonProjectPatch, JsonProjectPatchNull, JsonUpdateProject, ProjectRole, Visibility},
};
use bencher_rbac::{Organization, Project, project::Permission};
use diesel::{
    BoolExpressionMethods as _, Connection as _, ExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, TextExpressionMethods as _,
};
use dropshot::HttpError;
use project_role::InsertProjectRole;
use regex::Regex;
use slog::Logger;

use crate::{
    ApiContext, auth_conn,
    context::{DbConnection, Rbac},
    error::{
        BencherResource, assert_parentage, forbidden_error, issue_error, resource_conflict_err,
        resource_not_found_err, resource_not_found_error, unauthorized_error,
    },
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
        sql::last_insert_rowid,
    },
    model::{
        organization::QueryOrganization,
        user::{auth::AuthUser, public::PublicUser},
    },
    schema::{self, project as project_table},
    write_conn,
};

use super::organization::OrganizationId;

pub mod benchmark;
pub mod branch;
pub mod measure;
pub mod metric;
pub mod metric_boundary;
pub mod plot;
pub mod project_role;
pub mod report;
pub mod testbed;
pub mod threshold;

crate::macros::typed_id::typed_id!(ProjectId);

#[expect(clippy::expect_used)]
static UNIQUE_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((\d+)\)$").expect("Failed to create regex for unique project suffix")
});

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = project_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QueryProject {
    pub id: ProjectId,
    pub uuid: ProjectUuid,
    pub organization_id: OrganizationId,
    pub name: ResourceName,
    pub slug: ProjectSlug,
    pub url: Option<Url>,
    pub visibility: Visibility,
    pub created: DateTime,
    pub modified: DateTime,
    pub deleted: Option<DateTime>,
}

impl QueryProject {
    fn_eq_resource_id!(project, ProjectResourceId);
    fn_from_resource_id!(project, Project, ProjectResourceId, not_deleted);

    fn_get!(project, ProjectId, not_deleted);
    fn_get_uuid!(project, ProjectId, ProjectUuid, not_deleted);
    fn_from_uuid!(
        organization_id,
        OrganizationId,
        project,
        ProjectUuid,
        Project,
        not_deleted
    );

    fn from_slug(conn: &mut DbConnection, slug: &ProjectSlug) -> Result<Self, HttpError> {
        schema::project::table
            .filter(schema::project::slug.eq(slug))
            .filter(schema::project::deleted.is_null())
            .first(conn)
            .map_err(resource_not_found_err!(Project, slug.clone()))
    }

    pub async fn get_or_create<NameFn>(
        log: &Logger,
        context: &ApiContext,
        public_user: &PublicUser,
        project: &ProjectResourceId,
        project_name_fn: NameFn,
    ) -> Result<Self, HttpError>
    where
        NameFn: FnOnce() -> Result<ResourceName, HttpError>,
    {
        let query_project = Self::from_resource_id(auth_conn!(context), project);

        let http_error = match query_project {
            Ok(project) => return Ok(project),
            Err(e) => e,
        };

        let project_slug = match project.clone() {
            ProjectResourceId::Uuid(_) => return Err(http_error),
            ProjectResourceId::Slug(slug) => slug,
        };

        Self::get_or_create_inner(log, context, public_user, project_name_fn, project_slug).await
    }

    pub async fn get_or_create_from_context<NameFn, SlugFn>(
        log: &Logger,
        context: &ApiContext,
        public_user: &PublicUser,
        project_name_fn: NameFn,
        project_slug_fn: SlugFn,
    ) -> Result<Self, HttpError>
    where
        NameFn: FnOnce() -> Result<ResourceName, HttpError>,
        SlugFn: FnOnce() -> Result<ProjectSlug, HttpError>,
    {
        let project_slug = project_slug_fn()?;
        if let Ok(query_project) = Self::from_slug(auth_conn!(context), &project_slug) {
            return Ok(query_project);
        }

        Self::get_or_create_inner(log, context, public_user, project_name_fn, project_slug).await
    }

    async fn get_or_create_inner<NameFn>(
        log: &Logger,
        context: &ApiContext,
        public_user: &PublicUser,
        project_name_fn: NameFn,
        project_slug: ProjectSlug,
    ) -> Result<Self, HttpError>
    where
        NameFn: FnOnce() -> Result<ResourceName, HttpError>,
    {
        let project_name = project_name_fn()?;
        match public_user {
            PublicUser::Public(remote_ip) => {
                slog::info!(log, "Creating on-the-fly project for public user"; "project_slug" => %project_slug, "remote_ip" => ?remote_ip);
                let query_organization = QueryOrganization::get_or_create_from_project(
                    context,
                    &project_name,
                    &project_slug,
                )
                .await?;
                // In most cases, there should only ever be one on-the-fly project here,
                // but check the rate limit just in case.
                #[cfg(feature = "plus")]
                InsertProject::rate_limit(context, &query_organization).await?;
                // Currently, there is no semantic importance to having the organization and project have the same UUID.
                // However, it seems like a good idea to keep them in sync for now.
                // It makes identifying on-the-fly unclaimed projects easier, even after they have been claimed.
                // This is okay since there should never be more than one project in an unclaimed "from project" organization.
                let insert_project = InsertProject::from_organization(
                    &query_organization,
                    project_name,
                    project_slug,
                );
                Self::create_inner(log, context, insert_project).await
            },
            PublicUser::Auth(auth_user) => {
                let query_organization =
                    QueryOrganization::get_or_create_from_user(context, auth_user).await?;
                #[cfg(feature = "plus")]
                InsertProject::rate_limit(context, &query_organization).await?;
                // The choice was either to relax the schema constraint to allow duplicate project names
                // or to append a number to the project name to ensure uniqueness.
                let name =
                    Self::unique_name(log, context, &query_organization, project_name).await?;
                let insert_project =
                    InsertProject::new(query_organization.id, name, project_slug, None, None);
                // If the user is authenticated, then we may have created a new personal organization for them.
                // If so then we need to reload the permissions.
                // This is unlikely to be the case going forward, but it is needed for backwards compatibility.
                let auth_user = auth_user.reload(auth_conn!(context))?;
                Self::create(
                    log,
                    context,
                    &auth_user,
                    &query_organization,
                    insert_project,
                )
                .await
            },
        }
    }

    async fn unique_name(
        log: &Logger,
        context: &ApiContext,
        query_organization: &QueryOrganization,
        project_name: ResourceName,
    ) -> Result<ResourceName, HttpError> {
        // ` (_)`
        const SPACE_PAREN_LEN: usize = 3;
        let max_name_len = ResourceName::MAX_LEN - usize::MAX.to_string().len() - SPACE_PAREN_LEN;

        // This needs to happen before we escape the project name
        // so we check the possibly truncated name for originality
        let name_str = if project_name.as_ref().len() > max_name_len {
            const ELLIPSES_LEN: usize = 3;
            // The max length for a `usize` is 20 characters,
            // so we don't have to worry about the number suffix being too long.
            let name = project_name
                .as_ref()
                .chars()
                .take(max_name_len - ELLIPSES_LEN)
                .chain(".".repeat(ELLIPSES_LEN).chars())
                .collect::<String>();
            slog::debug!(log, "Truncated project name: {name}");
            name
        } else {
            project_name.to_string()
        };

        // Escape the project name for use in a LIKE pattern
        // https://www.sqlite.org/lang_expr.html#the_like_glob_regexp_match_and_extract_operators
        let escaped_name = name_str.replace('%', r"\%").replace('_', r"\_");
        // Create a regex pattern to match the original project name or any subsequent projects with the same name
        let pattern = format!("{escaped_name} (%)");
        slog::debug!(log, "LIKE pattern: {pattern}");

        // Include soft-deleted projects to avoid name collisions if they are restored.
        let Ok(highest_name) = schema::project::table
            .filter(schema::project::organization_id.eq(query_organization.id))
            .filter(
                schema::project::name
                    .eq(&project_name)
                    .or(schema::project::name.like(&pattern)),
            )
            .select(schema::project::name)
            .order(schema::project::name.desc())
            .first::<ResourceName>(auth_conn!(context))
        else {
            // The project name is already unique
            slog::debug!(log, "Project name is unique: {project_name}");
            return Ok(project_name);
        };

        let next_number = if highest_name == project_name {
            slog::debug!(log, "First project name duplicate: {highest_name}");
            1
        } else if let Some(caps) = UNIQUE_SUFFIX.captures(highest_name.as_ref()) {
            let last_number: usize = caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .ok_or_else(|| {
                    issue_error(
                        "Failed to parse project number",
                        &format!("Failed to parse number from project ({highest_name})"),
                        highest_name,
                    )
                })?;
            slog::debug!(log, "Multiple project name duplicates: {last_number}");
            last_number + 1
        } else {
            return Err(issue_error(
                "Failed to create new project number",
                &format!(
                    "Failed to create new number for project ({project_name}) with highest project ({highest_name})"
                ),
                highest_name,
            ));
        };

        let name_with_suffix = format!("{name_str} ({next_number})");
        slog::debug!(log, "Unique project name: {name_with_suffix}");
        name_with_suffix.parse().map_err(|e| {
            issue_error(
                "Failed to create new project name",
                &format!("Failed to create new project name ({name_with_suffix})",),
                e,
            )
        })
    }

    pub async fn create(
        log: &Logger,
        context: &ApiContext,
        auth_user: &AuthUser,
        query_organization: &QueryOrganization,
        insert_project: InsertProject,
    ) -> Result<Self, HttpError> {
        // Check to see if user has permission to create a project within the organization
        context
            .rbac
            .is_allowed_organization(
                auth_user,
                bencher_rbac::organization::Permission::Create,
                query_organization,
            )
            .map_err(forbidden_error)?;

        let timestamp = DateTime::now();
        let user_id = auth_user.id();

        let query_project = {
            let conn = write_conn!(context);
            conn.transaction(|conn| {
                let id = Self::insert(conn, &insert_project)?;

                // Connect the user to the project as a `Maintainer`
                let insert_proj_role = InsertProjectRole {
                    user_id,
                    project_id: id,
                    role: ProjectRole::Maintainer,
                    created: timestamp,
                    modified: timestamp,
                };
                diesel::insert_into(schema::project_role::table)
                    .values(&insert_proj_role)
                    .execute(conn)?;

                diesel::QueryResult::Ok(id)
            })
            .map_err(resource_conflict_err!(Project, &insert_project))
            .map(|id| insert_project.into_query(id))?
        };
        slog::debug!(log, "Created project: {query_project:?}");

        #[cfg(feature = "plus")]
        context.update_index(log, &query_project).await;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ProjectCreate);

        Ok(query_project)
    }

    async fn create_inner(
        log: &Logger,
        context: &ApiContext,
        insert_project: InsertProject,
    ) -> Result<Self, HttpError> {
        let query_project = {
            let conn = write_conn!(context);
            conn.transaction(|conn| Self::insert(conn, &insert_project))
                .map_err(resource_conflict_err!(Project, &insert_project))
                .map(|id| insert_project.into_query(id))?
        };
        slog::debug!(log, "Created project: {query_project:?}");

        #[cfg(feature = "plus")]
        context.update_index(log, &query_project).await;

        Ok(query_project)
    }

    fn insert(
        conn: &mut DbConnection,
        insert_project: &InsertProject,
    ) -> diesel::QueryResult<ProjectId> {
        diesel::insert_into(project_table::table)
            .values(insert_project)
            .execute(conn)?;
        diesel::select(last_insert_rowid()).get_result(conn)
    }

    pub fn organization(&self, conn: &mut DbConnection) -> Result<QueryOrganization, HttpError> {
        QueryOrganization::get(conn, self.organization_id)
    }

    pub fn is_public(&self) -> bool {
        self.visibility.is_public()
    }

    #[cfg(not(feature = "plus"))]
    pub fn is_visibility_public(visibility: Visibility) -> Result<(), HttpError> {
        visibility
            .is_public()
            .then_some(())
            .ok_or(crate::error::payment_required_error(
                "Private projects are only available with the an active Bencher Plus plan. Please upgrade your plan at: https://bencher.dev/pricing".to_owned()
            ))
    }

    pub fn is_allowed(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ProjectResourceId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        // Do not leak information about private projects.
        // Always return the same error.
        Self::is_allowed_inner(conn, rbac, project, auth_user, permission)
            .map_err(|_e| resource_not_found_error(BencherResource::Project, project, permission))
    }

    fn is_allowed_inner(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ProjectResourceId,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<Self, HttpError> {
        let query_project = Self::from_resource_id(conn, project)?;
        query_project.try_allowed(rbac, auth_user, permission)?;
        Ok(query_project)
    }

    pub fn is_allowed_public(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ProjectResourceId,
        public_user: &PublicUser,
    ) -> Result<Self, HttpError> {
        // Do not leak information about private projects.
        // Always return the same error.
        Self::is_allowed_public_inner(conn, rbac, project, public_user).map_err(|_e| {
            resource_not_found_error(BencherResource::Project, project, Permission::View)
        })
    }

    fn is_allowed_public_inner(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ProjectResourceId,
        public_user: &PublicUser,
    ) -> Result<Self, HttpError> {
        let query_project = Self::from_resource_id(conn, project)?;
        // Check to see if the project is public
        // If so, anyone can access it
        if query_project.is_public() {
            Ok(query_project)
        } else if let PublicUser::Auth(auth_user) = public_user {
            // If there is an `AuthUser` then validate access
            // Verify that the user is allowed
            query_project.try_allowed(rbac, auth_user, Permission::View)?;
            Ok(query_project)
        } else {
            Err(unauthorized_error(project))
        }
    }

    pub fn try_allowed(
        &self,
        rbac: &Rbac,
        auth_user: &AuthUser,
        permission: Permission,
    ) -> Result<(), HttpError> {
        rbac.is_allowed_project(auth_user, permission, self)
            .map_err(forbidden_error)
    }

    #[cfg(feature = "plus")]
    pub fn perf_url(&self, console_url: &url::Url) -> Result<Option<url::Url>, HttpError> {
        if !self.is_public() {
            return Ok(None);
        }
        let path = format!("/perf/{}", self.slug);
        console_url
            .join(&path)
            .map_err(|e| {
                issue_error(
                    "Failed to create new perf URL.",
                    &format!("Failed to create new perf URL for {console_url} at {path}",),
                    e,
                )
            })
            .map(Some)
    }

    /// Soft-delete this project: set the `deleted` timestamp and replace
    /// the name/slug with valid `Deleted {uuid}` / `deleted-{uuid}` sentinels
    /// to free the UNIQUE constraints for reuse.
    /// Also cleans up the search index (Plus feature).
    pub async fn soft_delete(
        &self,
        context: &ApiContext,
        log: &Logger,
        now: DateTime,
    ) -> Result<(), HttpError> {
        slog::info!(log, "Soft-deleting project: {}", self.uuid);
        let deleted_name: ResourceName = format!("Deleted {}", self.uuid).parse().map_err(|e| {
            issue_error(
                "Failed to create deleted project name",
                &format!("Project: {}", self.uuid),
                e,
            )
        })?;
        let deleted_slug: ProjectSlug = format!("deleted-{}", self.uuid).parse().map_err(|e| {
            issue_error(
                "Failed to create deleted project slug",
                &format!("Project: {}", self.uuid),
                e,
            )
        })?;
        diesel::update(schema::project::table.filter(schema::project::id.eq(self.id)))
            .set((
                schema::project::deleted.eq(now),
                schema::project::modified.eq(now),
                schema::project::name.eq(deleted_name),
                schema::project::slug.eq(deleted_slug),
            ))
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Project, self))?;

        #[cfg(feature = "plus")]
        context.delete_index(log, self).await;

        Ok(())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonProject, HttpError> {
        let query_organization = self.organization(conn)?;
        Ok(self.into_json_for_organization(conn, &query_organization))
    }

    pub fn into_json_for_organization(
        self,
        conn: &mut DbConnection,
        query_organization: &QueryOrganization,
    ) -> JsonProject {
        let Self {
            uuid,
            organization_id,
            name,
            slug,
            url,
            visibility,
            created,
            modified,
            ..
        } = self;
        assert_parentage(
            BencherResource::Organization,
            query_organization.id,
            BencherResource::Project,
            organization_id,
        );
        let claimed = query_organization.claimed_at(conn).ok();
        JsonProject {
            uuid,
            organization: query_organization.uuid,
            name,
            slug,
            url,
            visibility,
            created,
            modified,
            claimed,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = project_table)]
pub struct InsertProject {
    pub uuid: ProjectUuid,
    pub organization_id: OrganizationId,
    pub name: ResourceName,
    pub slug: ProjectSlug,
    pub url: Option<Url>,
    pub visibility: Visibility,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertProject {
    #[cfg(feature = "plus")]
    pub async fn rate_limit(
        context: &ApiContext,
        query_organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        use crate::context::RateLimitingError;

        let is_claimed = query_organization.is_claimed(auth_conn!(context))?;

        let resource = BencherResource::Project;
        let (start_time, end_time) = context.rate_limiting.window();
        // Include soft-deleted projects to prevent gaming rate limits via delete and recreate.
        let window_usage: u32 = schema::project::table
                .filter(schema::project::organization_id.eq(query_organization.id))
                .filter(schema::project::created.ge(start_time))
                .filter(schema::project::created.le(end_time))
                .count()
                .get_result::<i64>(auth_conn!(context))
                .map_err(resource_not_found_err!(Project, (query_organization, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for organization ({uuid}) between {start_time} and {end_time}.", uuid = query_organization.uuid),
                    e
                    )}
                )?;

        context.rate_limiting.check_claimable_limit(
            is_claimed,
            window_usage,
            |rate_limit| RateLimitingError::Organization {
                organization: query_organization.clone(),
                resource,
                rate_limit,
            },
            |rate_limit| RateLimitingError::Organization {
                organization: query_organization.clone(),
                resource,
                rate_limit,
            },
        )
    }

    pub fn new(
        organization_id: OrganizationId,
        name: ResourceName,
        slug: ProjectSlug,
        url: Option<Url>,
        visibility: Option<Visibility>,
    ) -> Self {
        Self::new_inner(
            ProjectUuid::new(),
            organization_id,
            name,
            slug,
            url,
            visibility,
        )
    }

    fn new_inner(
        uuid: ProjectUuid,
        organization_id: OrganizationId,
        name: ResourceName,
        slug: ProjectSlug,
        url: Option<Url>,
        visibility: Option<Visibility>,
    ) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid,
            organization_id,
            name,
            slug,
            url,
            visibility: visibility.unwrap_or_default(),
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn into_query(self, id: ProjectId) -> QueryProject {
        let Self {
            uuid,
            organization_id,
            name,
            slug,
            url,
            visibility,
            created,
            modified,
        } = self;
        QueryProject {
            id,
            uuid,
            organization_id,
            name,
            slug,
            url,
            visibility,
            created,
            modified,
            deleted: None,
        }
    }

    pub fn from_json(
        conn: &mut DbConnection,
        organization: &QueryOrganization,
        project: JsonNewProject,
    ) -> Self {
        let JsonNewProject {
            name,
            slug,
            url,
            visibility,
        } = project;
        let slug = ok_slug!(conn, &name, slug, project, QueryProject);
        Self::new(organization.id, name, slug, url, visibility)
    }

    fn from_organization(
        query_organization: &QueryOrganization,
        name: ResourceName,
        slug: ProjectSlug,
    ) -> Self {
        Self::new_inner(
            query_organization.uuid.into(),
            query_organization.id,
            name,
            slug,
            None,
            None,
        )
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = project_table)]
pub struct UpdateProject {
    pub name: Option<ResourceName>,
    pub slug: Option<ProjectSlug>,
    pub url: Option<Option<Url>>,
    pub visibility: Option<Visibility>,
    pub modified: DateTime,
}

impl From<JsonUpdateProject> for UpdateProject {
    fn from(update: JsonUpdateProject) -> Self {
        match update {
            JsonUpdateProject::Patch(patch) => {
                let JsonProjectPatch {
                    name,
                    slug,
                    url,
                    visibility,
                } = patch;
                Self {
                    name,
                    slug,
                    url: url.map(Some),
                    visibility,
                    modified: DateTime::now(),
                }
            },
            JsonUpdateProject::Null(patch_url) => {
                let JsonProjectPatchNull {
                    name,
                    slug,
                    url: (),
                    visibility,
                } = patch_url;
                Self {
                    name,
                    slug,
                    url: Some(None),
                    visibility,
                    modified: DateTime::now(),
                }
            },
        }
    }
}

impl From<&InsertProject> for Organization {
    fn from(project: &InsertProject) -> Self {
        Organization {
            id: project.organization_id.to_string(),
        }
    }
}

impl From<&QueryProject> for Organization {
    fn from(project: &QueryProject) -> Self {
        Organization {
            id: project.organization_id.to_string(),
        }
    }
}

impl From<&QueryProject> for Project {
    fn from(project: &QueryProject) -> Self {
        Project {
            id: project.id.to_string(),
            organization_id: project.organization_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use super::ProjectId;
    use crate::{
        macros::sql::last_insert_rowid,
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    #[test]
    fn last_insert_rowid_returns_project_id() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::project::table)
                    .values((
                        schema::project::uuid.eq(uuid),
                        schema::project::organization_id.eq(base.organization_id),
                        schema::project::name.eq("Project 1"),
                        schema::project::slug.eq("project-1"),
                        schema::project::visibility.eq(0),
                        schema::project::created.eq(DateTime::TEST),
                        schema::project::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<ProjectId>(conn)?;
                let select_id: ProjectId = schema::project::table
                    .filter(schema::project::uuid.eq(uuid))
                    .select(schema::project::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_project() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Insert first
        diesel::insert_into(schema::project::table)
            .values((
                schema::project::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::project::organization_id.eq(base.organization_id),
                schema::project::name.eq("Project 1"),
                schema::project::slug.eq("project-1"),
                schema::project::visibility.eq(0),
                schema::project::created.eq(DateTime::TEST),
                schema::project::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first project");

        // Insert second + verify
        let second_uuid = "00000000-0000-0000-0000-000000000011";
        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::project::table)
                    .values((
                        schema::project::uuid.eq(second_uuid),
                        schema::project::organization_id.eq(base.organization_id),
                        schema::project::name.eq("Project 2"),
                        schema::project::slug.eq("project-2"),
                        schema::project::visibility.eq(0),
                        schema::project::created.eq(DateTime::TEST),
                        schema::project::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<ProjectId>(conn)?;
                let select_id: ProjectId = schema::project::table
                    .filter(schema::project::uuid.eq(second_uuid))
                    .select(schema::project::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);

        let first_id: ProjectId = schema::project::table
            .filter(schema::project::uuid.eq("00000000-0000-0000-0000-000000000010"))
            .select(schema::project::id)
            .first(&mut conn)
            .expect("Failed to get first project id");
        assert_ne!(rowid, first_id);
    }
}
