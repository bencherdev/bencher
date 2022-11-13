use std::{str::FromStr, sync::Arc};

use bencher_json::{JsonNewTestbed, JsonResult, JsonTestbed, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        testbed::{InsertTestbed, QueryTestbed},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::{database_map, into_json},
        resource_id::fn_resource_id,
    },
    ApiError,
};

use super::Resource;

const RESULT_RESOURCE: Resource = Resource::Result;

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub project: ResourceId,
    pub result: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/results/{result}",
    tags = ["projects", "results"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/results/{result}",
    tags = ["projects", "results"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonResult>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(RESULT_RESOURCE, Method::GetOne);

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
) -> Result<JsonResult, ApiError> {
    let api_context = &mut *context.lock().await;
    let project_id =
        QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?.id;
    let conn = &mut api_context.database;

    schema::perf::table
        .filter(schema::perf::uuid.eq(path_params.result.to_string()))
        .inner_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::benchmark::project_id.eq(project_id))
        .inner_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .select((
            schema::perf::uuid,
            schema::report::uuid,
            schema::perf::iteration,
            schema::benchmark::uuid,
        ))
        .first::<(String, String, i32, String)>(conn)
        .map(
            |(perf_uuid, report_uuid, iteration, benchmark_uuid)| -> Result<_, ApiError> {
                Ok(JsonResult {
                    uuid: Uuid::from_str(&perf_uuid)?,
                    report: Uuid::from_str(&report_uuid)?,
                    iteration: iteration as u32,
                    benchmark: Uuid::from_str(&benchmark_uuid)?,
                })
            },
        )
        .map_err(api_error!())?
        .map_err(api_error!())
}
