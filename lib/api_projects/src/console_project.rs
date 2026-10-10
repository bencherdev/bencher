use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    ProjectResourceId,
    project::console_project::{
        JsonConsoleOrganization, JsonConsolePermissions, JsonConsoleProject,
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{issue_error, with_token_hint},
    model::{
        project::QueryProject,
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::alerts::active_count;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console",
    tags = ["projects"],
    unpublished = true,
}]
pub async fn proj_console_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a project as the console's shell shows it
///
/// The project, its organization, what the reader may do with it, and its active
/// alert count, in one request whose statement count does not grow.
/// The reader must be signed in, and the project must be public or the reader
/// must have `view` permissions for it.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console",
    tags = ["projects"],
    unpublished = true,
}]
pub async fn proj_console_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjConsoleParams>,
) -> Result<ResponseOk<JsonConsoleProject>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjConsoleParams,
    auth_user: &AuthUser,
) -> Result<JsonConsoleProject, HttpError> {
    auth_conn!(context, |conn| {
        let query_project = QueryProject::from_resource_id(conn, &path_params.project)?;
        // Permissions come from the roles the token loaded, so they cost no statement.
        let allowed = |permission| {
            query_project
                .try_allowed(&context.rbac, auth_user, permission)
                .is_ok()
        };
        // A public project is open to every signed in reader, member or not.
        let view = query_project.is_public() || allowed(Permission::View);
        if !view {
            return Err(query_project
                .auth_state_authenticated()
                .auth_error(&path_params.project, Permission::View));
        }
        #[cfg(feature = "plus")]
        context.rate_limiting.project_request(query_project.uuid)?;

        let permissions = JsonConsolePermissions {
            view,
            create: allowed(Permission::Create),
            edit: allowed(Permission::Edit),
            delete: allowed(Permission::Delete),
            manage: allowed(Permission::Manage),
        };
        let active_alerts = active_count(conn, &query_project).map_err(|e| {
            issue_error(
                "Failed to count active alerts",
                &format!(
                    "Failed to count active alerts for project ({}).",
                    query_project.uuid
                ),
                e,
            )
        })?;
        let query_organization = query_project.organization(conn)?;
        let organization = JsonConsoleOrganization {
            uuid: query_organization.uuid,
            name: query_organization.name.clone(),
            slug: query_organization.slug.clone(),
        };
        Ok(JsonConsoleProject {
            project: query_project.into_json_for_organization(conn, &query_organization),
            organization,
            permissions,
            active_alerts: u32::try_from(active_alerts).unwrap_or(u32::MAX),
        })
    })
}
