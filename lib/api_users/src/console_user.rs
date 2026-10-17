use std::collections::HashMap;

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BmfVersion, DateTime, OrganizationSlug, OrganizationUuid, ProjectSlug, ProjectUuid,
    ResourceName, UserResourceId, UserUuid,
    project::{alert::AlertStatus, console_project::JsonConsoleOrganization},
    user::console_user::{JsonConsoleUser, JsonConsoleUserProject},
};
use bencher_schema::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::issue_error,
    model::{
        organization::OrganizationId,
        project::{ProjectId, threshold::alert::archived_filter},
        user::{
            QueryUser,
            auth::{AuthUser, BearerToken},
            same_user,
        },
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    dsl::{count_star, max},
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct UserConsoleParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/console/users/{user}",
    tags = ["users"],
    unpublished = true,
}]
pub async fn user_console_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserConsoleParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a user as the console shows its reader
///
/// The user, their organizations, and the projects of those organizations, in one
/// request whose statement count does not grow.
/// Only the authenticated user themselves and server admins have access to this endpoint.
#[endpoint {
    method = GET,
    path =  "/v0/console/users/{user}",
    tags = ["users"],
    unpublished = true,
}]
pub async fn user_console_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserConsoleParams>,
) -> Result<ResponseOk<JsonConsoleUser>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: UserConsoleParams,
    auth_user: &AuthUser,
) -> Result<JsonConsoleUser, HttpError> {
    auth_conn!(context, |conn| {
        let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;
        same_user!(auth_user, context.rbac, query_user.uuid);

        let organizations = schema::organization::table
            .inner_join(schema::organization_role::table)
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization::deleted.is_null())
            .select((
                schema::organization::id,
                schema::organization::uuid,
                schema::organization::name,
                schema::organization::slug,
            ))
            .load::<(
                OrganizationId,
                OrganizationUuid,
                ResourceName,
                OrganizationSlug,
            )>(conn)
            .map_err(|e| {
                issue_error(
                    "Failed to list organizations",
                    &format!(
                        "Failed to list organizations for user ({}).",
                        query_user.uuid
                    ),
                    e,
                )
            })?;
        let organization_ids = organizations
            .iter()
            .map(|(id, _, _, _)| *id)
            .collect::<Vec<_>>();

        // One seek per project on its end time index.
        let last_report = schema::report::table
            .filter(schema::report::project_id.eq(schema::project::id))
            .select(max(schema::report::end_time))
            .single_value();
        let projects = schema::project::table
            .inner_join(schema::organization::table)
            .filter(schema::project::organization_id.eq_any(&organization_ids))
            .filter(schema::project::deleted.is_null())
            .select((
                schema::project::id,
                schema::project::uuid,
                schema::project::name,
                schema::project::slug,
                schema::organization::uuid,
                schema::project::bmf_version,
                last_report,
            ))
            .load::<ProjectRow>(conn)
            .map_err(|e| {
                issue_error(
                    "Failed to list projects",
                    &format!("Failed to list projects for user ({}).", query_user.uuid),
                    e,
                )
            })?;

        let project_ids = projects.iter().map(|project| project.0).collect::<Vec<_>>();
        let active_alerts = count_active_alerts(conn, &project_ids, query_user.uuid)?;

        let projects = projects
            .into_iter()
            .map(
                |(id, uuid, name, slug, organization, bmf_version, last_report)| {
                    JsonConsoleUserProject {
                        uuid,
                        name,
                        slug,
                        organization,
                        bmf_version,
                        last_report: last_report.map(Into::into),
                        active_alerts: active_alerts
                            .get(&id)
                            .map_or(0, |count| u32::try_from(*count).unwrap_or(u32::MAX)),
                    }
                },
            )
            .collect();
        let QueryUser {
            uuid, name, slug, ..
        } = query_user;
        Ok(JsonConsoleUser {
            uuid,
            name,
            slug,
            organizations: organizations
                .into_iter()
                .map(|(_, uuid, name, slug)| JsonConsoleOrganization { uuid, name, slug })
                .collect(),
            projects,
        })
    })
}

/// The active alerts of each project that has any, counted as its Alerts list counts them.
fn count_active_alerts(
    conn: &mut DbConnection,
    project_ids: &[ProjectId],
    user: UserUuid,
) -> Result<HashMap<ProjectId, i64>, HttpError> {
    schema::alert::table
        .inner_join(
            schema::threshold::table
                .inner_join(schema::branch::table)
                .inner_join(schema::testbed::table)
                .inner_join(schema::measure::table),
        )
        .filter(schema::alert::project_id.eq_any(project_ids))
        .filter(schema::alert::status.eq(AlertStatus::Active))
        .filter(archived_filter(None))
        .group_by(schema::alert::project_id)
        .select((schema::alert::project_id, count_star()))
        .load::<(ProjectId, i64)>(conn)
        .map(|counts| counts.into_iter().collect())
        .map_err(|e| {
            issue_error(
                "Failed to count active alerts",
                &format!("Failed to count active alerts for user ({user})."),
                e,
            )
        })
}

type ProjectRow = (
    ProjectId,
    ProjectUuid,
    ResourceName,
    ProjectSlug,
    OrganizationUuid,
    BmfVersion,
    Option<DateTime>,
);
