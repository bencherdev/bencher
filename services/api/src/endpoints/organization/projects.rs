use bencher_json::{
    JsonDirection, JsonNewProject, JsonPagination, JsonProject, JsonProjects, NonEmpty, ResourceId,
};
use bencher_rbac::{organization::Permission, project::Role};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        organization::QueryOrganization,
        project::{
            branch::{InsertBranch, QueryBranch},
            metric_kind::{InsertMetricKind, QueryMetricKind},
            project_role::InsertProjectRole,
            testbed::{InsertTestbed, QueryTestbed},
            threshold::InsertThreshold,
            InsertProject, QueryProject,
        },
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[derive(Deserialize, JsonSchema)]
pub struct OrgProjectsParams {
    pub organization: ResourceId,
}

pub type OrgProjectsPagination = JsonPagination<OrgProjectsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgProjectsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgProjectsQuery {
    pub name: Option<NonEmpty>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_projects_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgProjectsParams>,
    _pagination_params: Query<OrgProjectsPagination>,
    _query_params: Query<OrgProjectsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_projects_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgProjectsParams>,
    pagination_params: Query<OrgProjectsPagination>,
    query_params: Query<OrgProjectsQuery>,
) -> Result<ResponseOk<JsonProjects>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: OrgProjectsParams,
    pagination_params: OrgProjectsPagination,
    query_params: OrgProjectsQuery,
    auth_user: &AuthUser,
    endpoint: Endpoint,
) -> Result<JsonProjects, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;

    let mut query = schema::project::table
        .filter(schema::project::organization_id.eq(query_organization.id))
        .into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::project::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        OrgProjectsSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::project::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::project::name.desc()),
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryProject>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_project_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgProjectsParams>,
    body: TypedBody<JsonNewProject>,
) -> Result<ResponseAccepted<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgProjectsParams,
    json_project: JsonNewProject,
    auth_user: &AuthUser,
) -> Result<JsonProject, ApiError> {
    let conn = &mut *context.conn().await;

    // Check project visibility
    #[cfg(not(feature = "plus"))]
    crate::model::project::visibility::project_visibility::project_visibility(
        json_project.visibility,
    )?;
    #[cfg(feature = "plus")]
    crate::model::project::visibility::project_visibility::project_visibility(
        conn,
        context.biller.as_ref(),
        &context.licensor,
        &path_params.organization,
        json_project.visibility,
    )
    .await?;

    // Create the project
    let insert_project = InsertProject::from_json(conn, &path_params.organization, json_project)?;

    // Check to see if user has permission to create a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Create, &insert_project)?;

    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(conn)
        .map_err(api_error!())?;
    let query_project = schema::project::table
        .filter(schema::project::uuid.eq(&insert_project.uuid))
        .first::<QueryProject>(conn)
        .map_err(api_error!())?;

    let timestamp = Utc::now().timestamp();
    // Connect the user to the project as a `Maintainer`
    let insert_proj_role = InsertProjectRole {
        user_id: auth_user.id,
        project_id: query_project.id,
        role: Role::Maintainer.to_string(),
        created: timestamp,
        modified: timestamp,
    };
    diesel::insert_into(schema::project_role::table)
        .values(&insert_proj_role)
        .execute(conn)
        .map_err(api_error!())?;

    // Add a `main` branch to the project
    let insert_branch = InsertBranch::main(conn, query_project.id);
    diesel::insert_into(schema::branch::table)
        .values(&insert_branch)
        .execute(conn)
        .map_err(api_error!())?;
    let branch_id = QueryBranch::get_id(conn, &insert_branch.uuid)?;

    // Add a `localhost` testbed to the project
    let insert_testbed = InsertTestbed::localhost(conn, query_project.id);
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn)
        .map_err(api_error!())?;
    let testbed_id = QueryTestbed::get_id(conn, &insert_testbed.uuid)?;

    // Add a `latency` metric kind to the project
    let insert_metric_kind = InsertMetricKind::latency(conn, query_project.id);
    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(api_error!())?;
    let metric_kind_id = QueryMetricKind::get_id(conn, &insert_metric_kind.uuid)?;
    // Add a `latency` threshold to the project
    InsertThreshold::upper_boundary(
        conn,
        query_project.id,
        metric_kind_id,
        branch_id,
        testbed_id,
    )?;

    // Add a `throughput` metric kind to the project
    let insert_metric_kind = InsertMetricKind::throughput(conn, query_project.id);
    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(api_error!())?;
    let metric_kind_id = QueryMetricKind::get_id(conn, &insert_metric_kind.uuid)?;
    // Add a `throughput` threshold to the project
    InsertThreshold::lower_boundary(
        conn,
        query_project.id,
        metric_kind_id,
        branch_id,
        testbed_id,
    )?;

    query_project.into_json(conn)
}
