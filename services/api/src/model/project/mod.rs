use std::{str::FromStr, string::ToString};

#[cfg(feature = "plus")]
use bencher_billing::SubscriptionId;
#[cfg(feature = "plus")]
use bencher_json::Jwt;
use bencher_json::{
    project::{JsonProjectPatch, JsonProjectPatchNull, JsonUpdateProject},
    JsonNewProject, JsonProject, NonEmpty, ResourceId, Slug, Url,
};
use bencher_rbac::{Organization, Project};
use chrono::Utc;
#[cfg(feature = "plus")]
use diesel::JoinOnDsl;
use diesel::{ExpressionMethods, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    context::{DbConnection, Rbac},
    error::{forbidden_error, resource_not_found_err, unauthorized_error},
    model::{organization::QueryOrganization, user::auth::AuthUser},
    schema::{self, project as project_table},
    util::{query::fn_get, resource_id::fn_resource_id, slug::unwrap_slug, to_date_time},
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

#[derive(diesel::Insertable)]
#[diesel(table_name = project_table)]
pub struct InsertProject {
    pub uuid: String,
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
            uuid: Uuid::new_v4().to_string(),
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
    pub uuid: String,
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
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            organization: QueryOrganization::get_uuid(conn, organization_id)?,
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

    pub fn get_uuid(conn: &mut DbConnection, id: ProjectId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::project::table
            .filter(schema::project::id.eq(id))
            .select(schema::project::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
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

    #[cfg(feature = "plus")]
    pub fn get_subscription(
        conn: &mut DbConnection,
        id: ProjectId,
    ) -> Result<Option<SubscriptionId>, ApiError> {
        let subscription: Option<String> = schema::organization::table
            .left_join(
                schema::project::table
                    .on(schema::organization::id.eq(schema::project::organization_id)),
            )
            .filter(schema::project::id.eq(id))
            .select(schema::organization::subscription)
            .first(conn)
            .map_err(ApiError::from)?;

        Ok(if let Some(subscription) = &subscription {
            Some(SubscriptionId::from_str(subscription)?)
        } else {
            None
        })
    }

    #[cfg(feature = "plus")]
    pub fn get_license(
        conn: &mut DbConnection,
        id: ProjectId,
    ) -> Result<Option<(Uuid, Jwt)>, ApiError> {
        let (uuid, license): (String, Option<String>) = schema::organization::table
            .left_join(
                schema::project::table
                    .on(schema::organization::id.eq(schema::project::organization_id)),
            )
            .filter(schema::project::id.eq(id))
            .select((schema::organization::uuid, schema::organization::license))
            .first(conn)
            .map_err(ApiError::from)?;

        Ok(if let Some(license) = &license {
            Some((Uuid::from_str(&uuid)?, Jwt::from_str(license)?))
        } else {
            None
        })
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
