use bencher_json::{
    project::JsonUpdateProject, JsonDirection, JsonEmpty, JsonPagination, JsonProject,
    JsonProjects, NonEmpty, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        project::{
            visibility::{project_visibility::project_visibility, Visibility},
            QueryProject, UpdateProject,
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

pub type ProjectsPagination = JsonPagination<ProjectsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectsSort {
    #[default]
    Name,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct ProjectsQuery {
    pub name: Option<NonEmpty>,
    pub public: Option<bool>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<ProjectsPagination>,
    _query_params: Query<ProjectsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_get(
    rqctx: RequestContext<ApiContext>,
    pagination_params: Query<ProjectsPagination>,
    query_params: Query<ProjectsQuery>,
) -> Result<ResponseOk<JsonProjects>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    pagination_params: ProjectsPagination,
    query_params: ProjectsQuery,
    endpoint: Endpoint,
) -> Result<JsonProjects, ApiError> {
    let conn = &mut *context.conn().await;

    let mut query = schema::project::table.into_boxed();

    // All users should just see the public projects if the query is for public projects
    if let Some(true) = query_params.public {
        query = query.filter(schema::project::visibility.eq(Visibility::Public as i32));
    } else if let Some(auth_user) = auth_user {
        if !auth_user.is_admin(&context.rbac) {
            let projects =
                auth_user.projects(&context.rbac, bencher_rbac::project::Permission::View);
            query = query.filter(schema::project::id.eq_any(projects));
        }
    } else {
        return Err(ApiError::PrivateProjects);
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::project::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        ProjectsSort::Name => match pagination_params.direction {
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

#[derive(Deserialize, JsonSchema)]
pub struct ProjectParams {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn project_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjectParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjectParams>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjectParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonProject, ApiError> {
    let conn = &mut *context.conn().await;

    QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?
        .into_json(conn)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn project_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjectParams>,
    body: TypedBody<JsonUpdateProject>,
) -> Result<ResponseAccepted<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::Patch);

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjectParams,
    json_project: JsonUpdateProject,
    auth_user: &AuthUser,
) -> Result<JsonProject, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    // Check project visibility
    #[cfg(not(feature = "plus"))]
    project_visibility(json_project.visibility())?;
    #[cfg(feature = "plus")]
    {
        let organization = crate::model::organization::QueryOrganization::get_uuid(
            conn,
            query_project.organization_id,
        )?
        .into();
        project_visibility(
            conn,
            context.biller.as_ref(),
            &context.licensor,
            &organization,
            json_project.visibility(),
        )
        .await?;
    }

    diesel::update(schema::project::table.filter(schema::project::id.eq(query_project.id)))
        .set(&UpdateProject::from(json_project))
        .execute(conn)
        .map_err(api_error!())?;

    QueryProject::get(conn, query_project.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjectParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::Delete);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjectParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        bencher_rbac::project::Permission::Delete,
    )?;
    info!("Deleting project: {:?}", query_project);

    diesel::delete(schema::project::table.filter(schema::project::id.eq(query_project.id)))
        .execute(conn)
        .map_err(api_error!())?;
    info!("Deleted project: {:?}", query_project);

    Ok(JsonEmpty {})
}
