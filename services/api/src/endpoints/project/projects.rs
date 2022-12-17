use std::sync::Arc;

use bencher_json::{JsonProject, ResourceId};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{project::QueryProject, user::auth::AuthUser},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn dir_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<ResponseOk<Vec<JsonProject>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(rqctx.context(), auth_user.as_ref(), endpoint)
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
    endpoint: Endpoint,
) -> Result<Vec<JsonProject>, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database;

    let mut sql = schema::project::table.into_boxed();

    if let Some(auth_user) = auth_user {
        if !auth_user.is_admin(&api_context.rbac) {
            let projects =
                auth_user.projects(&api_context.rbac, bencher_rbac::project::Permission::View);
            sql = sql.filter(
                schema::project::public
                    .eq(true)
                    .or(schema::project::id.eq_any(projects)),
            );
        }
    } else {
        sql = sql.filter(schema::project::public.eq(true));
    }

    Ok(sql
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

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
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
    rqctx: Arc<RequestContext<Context>>,
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
        .into_json(&mut api_context.database)
}
