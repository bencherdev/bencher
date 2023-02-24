use std::{str::FromStr, string::ToString};

#[cfg(feature = "plus")]
use bencher_billing::SubscriptionId;
#[cfg(feature = "plus")]
use bencher_json::Jwt;
use bencher_json::{
    project::JsonVisibility, JsonNewProject, JsonProject, NonEmpty, ResourceId, Slug, Url,
};
use bencher_rbac::{Organization, Project};
#[cfg(feature = "plus")]
use diesel::JoinOnDsl;
use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use crate::{
    context::ApiContext,
    diesel::ExpressionMethods,
    error::api_error,
    model::{organization::QueryOrganization, user::auth::AuthUser},
    schema::{self, project as project_table},
    util::{resource_id::fn_resource_id, slug::unwrap_slug},
    ApiError,
};

use self::visibility::Visibility;

pub mod benchmark;
pub mod branch;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod project_role;
pub mod report;
pub mod testbed;
pub mod threshold;
pub mod version;
pub mod visibility;

#[derive(Insertable)]
#[diesel(table_name = project_table)]
pub struct InsertProject {
    pub uuid: String,
    pub organization_id: i32,
    pub name: String,
    pub slug: String,
    pub url: Option<String>,
    pub visibility: i32,
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
            url,
            visibility,
        } = project;
        let slug = unwrap_slug!(conn, name.as_ref(), slug, project, QueryProject);
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            organization_id: QueryOrganization::from_resource_id(conn, organization)?.id,
            name: name.into(),
            slug,
            url: url.map(|u| u.to_string()),
            visibility: Visibility::from(visibility.unwrap_or_default()) as i32,
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
    pub url: Option<String>,
    pub visibility: i32,
}

impl QueryProject {
    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonProject, ApiError> {
        let Self {
            uuid,
            organization_id,
            name,
            slug,
            url,
            visibility,
            ..
        } = self;
        Ok(JsonProject {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            organization: QueryOrganization::get_uuid(conn, organization_id)?,
            name: NonEmpty::from_str(&name)?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
            url: ok_url(url.as_deref())?,
            visibility: Visibility::try_from(visibility)?.into(),
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

    #[cfg(feature = "plus")]
    pub fn is_public(conn: &mut SqliteConnection, id: i32) -> Result<bool, ApiError> {
        is_public(
            schema::project::table
                .filter(schema::project::id.eq(id))
                .select(schema::project::visibility)
                .first::<i32>(conn)?,
        )
    }

    #[cfg(feature = "plus")]
    pub fn get_subscription(
        conn: &mut SqliteConnection,
        id: i32,
    ) -> Result<Option<SubscriptionId>, ApiError> {
        let subscription: Option<String> = schema::organization::table
            .left_join(
                schema::project::table
                    .on(schema::organization::id.eq(schema::project::organization_id)),
            )
            .filter(schema::project::id.eq(id))
            .select(schema::organization::subscription)
            .first(conn)
            .map_err(api_error!())?;

        Ok(if let Some(subscription) = &subscription {
            Some(SubscriptionId::from_str(subscription)?)
        } else {
            None
        })
    }

    #[cfg(feature = "plus")]
    pub fn get_license(
        conn: &mut SqliteConnection,
        id: i32,
    ) -> Result<Option<(Uuid, Jwt)>, ApiError> {
        let (uuid, license): (String, Option<String>) = schema::organization::table
            .left_join(
                schema::project::table
                    .on(schema::organization::id.eq(schema::project::organization_id)),
            )
            .filter(schema::project::id.eq(id))
            .select((schema::organization::uuid, schema::organization::license))
            .first(conn)
            .map_err(api_error!())?;

        Ok(if let Some(license) = &license {
            Some((Uuid::from_str(&uuid)?, Jwt::from_str(license)?))
        } else {
            None
        })
    }

    pub fn is_allowed_resource_id(
        api_context: &mut ApiContext,
        project: &ResourceId,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
    ) -> Result<Self, ApiError> {
        let query_project =
            QueryProject::from_resource_id(&mut api_context.database.connection, project)?;

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
            .first(&mut api_context.database.connection)
            .map_err(api_error!())?;

        api_context
            .rbac
            .is_allowed_project(auth_user, permission, &query_project)?;

        Ok(query_project)
    }

    pub fn is_allowed_public(
        api_context: &mut ApiContext,
        project: &ResourceId,
        auth_user: Option<&AuthUser>,
    ) -> Result<Self, ApiError> {
        // Get the project
        let project =
            QueryProject::from_resource_id(&mut api_context.database.connection, project)?;

        // Check to see if the project is public
        // If so, anyone can access it
        if is_public(project.visibility)? {
            Ok(project)
        } else if let Some(auth_user) = auth_user {
            // If there is an `AuthUser` then validate access
            // Verify that the user is allowed
            QueryProject::is_allowed_id(
                api_context,
                project.id,
                auth_user,
                bencher_rbac::project::Permission::View,
            )
        } else {
            Err(ApiError::PrivateProject(project.id))
        }
    }
}

fn ok_url(url: Option<&str>) -> Result<Option<Url>, ApiError> {
    Ok(if let Some(url) = url {
        Some(Url::from_str(url).map_err(api_error!())?)
    } else {
        None
    })
}

fn is_public(visibility: i32) -> Result<bool, ApiError> {
    Visibility::try_from(visibility)
        .map(From::from)
        .map(|v: JsonVisibility| v.is_public())
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
