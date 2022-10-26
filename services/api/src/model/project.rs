use std::{str::FromStr, string::ToString};

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use bencher_rbac::{Organization, Project};
use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use url::Url;
use uuid::Uuid;

use super::{organization::QueryOrganization, user::auth::AuthUser};
use crate::{
    diesel::ExpressionMethods,
    error::api_error,
    schema::{self, project as project_table},
    util::{map_http_error, resource_id::fn_resource_id, slug::unwrap_slug, ApiContext},
    ApiError,
};

#[derive(Insertable)]
#[diesel(table_name = project_table)]
pub struct InsertProject {
    pub uuid: String,
    pub organization_id: i32,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub public: bool,
}

impl InsertProject {
    pub fn from_json(
        conn: &mut SqliteConnection,
        organization: &ResourceId,
        project: JsonNewProject,
    ) -> Result<Self, ApiError> {
        let JsonNewProject {
            name,
            slug,
            description,
            url,
            public,
        } = project;
        let slug = unwrap_slug!(conn, &name, slug, project, QueryProject);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            organization_id: QueryOrganization::from_resource_id(conn, organization)?.id,
            name,
            slug,
            description,
            url: url.map(|u| u.to_string()),
            public,
        })
    }
}

fn_resource_id!(project);

#[derive(Debug, Clone, Queryable)]
pub struct QueryProject {
    pub id: i32,
    pub uuid: String,
    pub organization_id: i32,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub public: bool,
}

impl QueryProject {
    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonProject, ApiError> {
        let Self {
            id: _,
            uuid,
            organization_id,
            name,
            slug,
            description,
            url,
            public,
        } = self;
        Ok(JsonProject {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            organization: QueryOrganization::get_uuid(conn, organization_id)?,
            name,
            slug,
            description,
            url: ok_url(url.as_deref())?,
            public,
        })
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        project: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::project::table
            .filter(resource_id(project)?)
            .first::<QueryProject>(conn)
            .map_err(api_error!())
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::project::table
            .filter(schema::project::id.eq(id))
            .select(schema::project::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn is_allowed_resource_id(
        api_context: &mut ApiContext,
        project: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
    ) -> Result<Self, ApiError> {
        let query_project = QueryProject::from_resource_id(&mut api_context.database, project)?;

        api_context
            .rbac
            .is_allowed_project(auth_user, permission, &query_project)?;

        Ok(query_project)
    }

    pub fn is_allowed_id(
        api_context: &mut ApiContext,
        project_id: i32,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
    ) -> Result<Self, ApiError> {
        let query_project = schema::project::table
            .filter(schema::project::id.eq(project_id))
            .first(&mut api_context.database)
            .map_err(api_error!())?;

        api_context
            .rbac
            .is_allowed_project(auth_user, permission, &query_project)?;

        Ok(query_project)
    }
}

fn ok_url(url: Option<&str>) -> Result<Option<Url>, HttpError> {
    Ok(if let Some(url) = url {
        Some(Url::parse(url).map_err(map_http_error!("Failed to get project."))?)
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
