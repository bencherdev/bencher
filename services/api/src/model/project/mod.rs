use std::{str::FromStr, string::ToString};

use bencher_json::{
    project::{JsonProjectPatch, JsonProjectPatchNull, JsonUpdateProject},
    JsonNewProject, JsonProject, NonEmpty, ResourceId, Slug, Url,
};
use bencher_rbac::{Organization, Project};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::{DbConnection, Rbac},
    error::{forbidden_error, resource_not_found_err, unauthorized_error},
    model::{organization::QueryOrganization, user::auth::AuthUser},
    schema::{self, project as project_table},
    util::{
        query::{fn_get, fn_get_uuid},
        resource_id::fn_resource_id,
        slug::unwrap_slug,
        to_date_time,
    },
    ApiError,
};

use self::visibility::Visibility;

use super::{organization::OrganizationId, user::auth::BEARER_TOKEN_FORMAT};

pub mod benchmark;
pub mod branch;
pub mod branch_version;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod project_role;
pub mod report;
pub mod testbed;
pub mod threshold;
pub mod version;
pub mod visibility;

crate::util::typed_id::typed_id!(ProjectId);
crate::util::typed_uuid::typed_uuid!(ProjectUuid);

#[derive(diesel::Insertable)]
#[diesel(table_name = project_table)]
pub struct InsertProject {
    pub uuid: ProjectUuid,
    pub organization_id: OrganizationId,
    pub name: String,
    pub slug: String,
    pub url: Option<String>,
    pub visibility: Visibility,
    pub created: i64,
    pub modified: i64,
}

impl InsertProject {
    pub fn from_json(
        conn: &mut DbConnection,
        organization: &ResourceId,
        project: JsonNewProject,
    ) -> Result<Self, ApiError> {
        let JsonNewProject {
            name,
            slug,
            url,
            visibility,
        } = project;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, project, QueryProject);
        let timestamp = Utc::now().timestamp();
        Ok(Self {
            uuid: ProjectUuid::new(),
            organization_id: QueryOrganization::from_resource_id(conn, organization)?.id,
            name: name.into(),
            slug,
            url: url.map(|u| u.to_string()),
            visibility: Visibility::from(visibility.unwrap_or_default()),
            created: timestamp,
            modified: timestamp,
        })
    }
}

fn_resource_id!(project);

#[derive(Debug, Clone, Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = project_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QueryProject {
    pub id: ProjectId,
    pub uuid: ProjectUuid,
    pub organization_id: OrganizationId,
    pub name: String,
    pub slug: String,
    pub url: Option<String>,
    pub visibility: Visibility,
    pub created: i64,
    pub modified: i64,
}

impl QueryProject {
    fn_get!(project);
    fn_get_uuid!(project, ProjectId, ProjectUuid);

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonProject, ApiError> {
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
        Ok(JsonProject {
            uuid: uuid.into(),
            organization: QueryOrganization::get_uuid(conn, organization_id)?.into(),
            name: NonEmpty::from_str(&name)?,
            slug: Slug::from_str(&slug).map_err(ApiError::from)?,
            url: ok_url(url.as_deref())?,
            visibility: visibility.into(),
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        project: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::project::table
            .filter(resource_id(project)?)
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Project, project.clone()))
    }

    #[cfg(feature = "plus")]
    pub fn is_public(conn: &mut DbConnection, id: ProjectId) -> Result<bool, ApiError> {
        Visibility::try_from(
            schema::project::table
                .filter(schema::project::id.eq(id))
                .select(schema::project::visibility)
                .first::<i32>(conn)?,
        )
        .map(Visibility::is_public)
    }

    pub fn is_allowed(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
    ) -> Result<Self, HttpError> {
        let query_project = Self::from_resource_id(conn, project)?;
        rbac.is_allowed_project(auth_user, permission, &query_project)
            .map_err(forbidden_error)?;
        Ok(query_project)
    }

    pub fn is_allowed_public(
        conn: &mut DbConnection,
        rbac: &Rbac,
        project: &ResourceId,
        auth_user: Option<&AuthUser>,
    ) -> Result<Self, HttpError> {
        let query_project = Self::from_resource_id(conn, project)?;
        // Check to see if the project is public
        // If so, anyone can access it
        if query_project.visibility.is_public() {
            Ok(query_project)
        } else if let Some(auth_user) = auth_user {
            // If there is an `AuthUser` then validate access
            // Verify that the user is allowed
            rbac.is_allowed_project(
                auth_user,
                bencher_rbac::project::Permission::View,
                &query_project,
            )
            .map_err(forbidden_error)?;
            Ok(query_project)
        } else {
            Err(unauthorized_error(format!(
                "Project ({query_project:?}) is not public and requires authentication.\n{BEARER_TOKEN_FORMAT}",
            )))
        }
    }
}

fn ok_url(url: Option<&str>) -> Result<Option<Url>, ApiError> {
    Ok(if let Some(url) = url {
        Some(Url::from_str(url).map_err(ApiError::from)?)
    } else {
        None
    })
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

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = project_table)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub url: Option<Option<String>>,
    pub visibility: Option<i32>,
    pub modified: i64,
}

impl From<JsonUpdateProject> for UpdateProject {
    fn from(update: JsonUpdateProject) -> Self {
        let (name, slug, url, visibility) = match update {
            JsonUpdateProject::Patch(patch) => {
                let JsonProjectPatch {
                    name,
                    slug,
                    url,
                    visibility,
                } = patch;
                (name, slug, url.map(Some), visibility)
            },
            JsonUpdateProject::Null(patch_url) => {
                let JsonProjectPatchNull {
                    name,
                    slug,
                    url: _,
                    visibility,
                } = patch_url;
                (name, slug, Some(None), visibility)
            },
        };
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            url: url.map(|url| url.map(Into::into)),
            visibility: visibility.map(|v| Visibility::from(v) as i32),
            modified: Utc::now().timestamp(),
        }
    }
}
