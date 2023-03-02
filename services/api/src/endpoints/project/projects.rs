use bencher_json::{project::JsonProjects, JsonProject, ResourceId};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        project::{visibility::Visibility, QueryProject},
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

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn dir_options(
    _rqctx: RequestContext<Context>,
    _query_params: Query<JsonProjects>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn get_ls(
    rqctx: RequestContext<Context>,
    query_params: Query<JsonProjects>,
) -> Result<ResponseOk<Vec<JsonProject>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
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
    context: &Context,
    auth_user: Option<&AuthUser>,
    json_projects: JsonProjects,
    endpoint: Endpoint,
) -> Result<Vec<JsonProject>, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database.connection;

    let mut query = schema::project::table.into_boxed();

    // All users should just see the public projects if the query is for public projects
    if let Some(true) = json_projects.public {
        query = query.filter(schema::project::visibility.eq(Visibility::Public as i32));
    } else if let Some(auth_user) = auth_user {
        if !auth_user.is_admin(&api_context.rbac) {
            let projects =
                auth_user.projects(&api_context.rbac, bencher_rbac::project::Permission::View);
            query = query.filter(schema::project::id.eq_any(projects));
        }
    } else {
        return Err(ApiError::PrivateProjects);
    }

    Ok(query
        .order((schema::project::name, schema::project::slug))
        .load::<QueryProject>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn get_one(
    rqctx: RequestContext<Context>,
    path_params: Path<OnePath>,
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
    context: &Context,
    path_params: OnePath,
    auth_user: Option<&AuthUser>,
) -> Result<JsonProject, ApiError> {
    let api_context = &mut *context.lock().await;
    QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?
        .into_json(&mut api_context.database.connection)
}
