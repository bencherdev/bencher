use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, ResponseCreated, ResponseDeleted, ResponseOk,
};
use bencher_json::{JsonNewRunnerSpec, JsonSpec, JsonSpecs, RunnerResourceId, SpecResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        runner::{InsertRunnerSpec, QueryRunner, QueryRunnerSpec},
        spec::QuerySpec,
        user::{admin::AdminUser, auth::BearerToken},
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct RunnerSpecsParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}/specs",
    tags = ["runners"]
}]
pub async fn runner_specs_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerSpecsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List specs for a runner
///
/// List all hardware specs associated with a runner.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = GET,
    path = "/v0/runners/{runner}/specs",
    tags = ["runners"]
}]
pub async fn runner_specs_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerSpecsParams>,
) -> Result<ResponseOk<JsonSpecs>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_ls_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: RunnerSpecsParams,
) -> Result<JsonSpecs, HttpError> {
    auth_conn!(context, |conn| {
        let query_runner = QueryRunner::from_resource_id(conn, &path_params.runner)?;
        let spec_ids = QueryRunnerSpec::spec_ids_for_runner(conn, query_runner.id)?;
        let json_specs: Vec<JsonSpec> = spec_ids
            .into_iter()
            .map(|spec_id| QuerySpec::get(conn, spec_id).map(QuerySpec::into_json))
            .collect::<Result<_, _>>()?;
        Ok(json_specs.into())
    })
}

/// Add a spec to a runner
///
/// Associate a hardware spec with a runner.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = POST,
    path = "/v0/runners/{runner}/specs",
    tags = ["runners"]
}]
pub async fn runner_specs_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerSpecsParams>,
    body: TypedBody<JsonNewRunnerSpec>,
) -> Result<ResponseCreated<JsonSpec>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: RunnerSpecsParams,
    json_runner_spec: JsonNewRunnerSpec,
) -> Result<JsonSpec, HttpError> {
    let query_runner = QueryRunner::from_resource_id(auth_conn!(context), &path_params.runner)?;
    let query_spec = QuerySpec::from_resource_id(auth_conn!(context), &json_runner_spec.spec)?;

    let insert = InsertRunnerSpec {
        runner_id: query_runner.id,
        spec_id: query_spec.id,
    };

    diesel::insert_into(schema::runner_spec::table)
        .values(&insert)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(RunnerSpec, insert))?;

    Ok(query_spec.into_json())
}

#[derive(Deserialize, JsonSchema)]
pub struct RunnerSpecParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
    /// The UUID or slug for a spec.
    pub spec: SpecResourceId,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}/specs/{spec}",
    tags = ["runners"]
}]
pub async fn runner_spec_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerSpecParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Delete.into()]))
}

/// Remove a spec from a runner
///
/// Remove the association between a hardware spec and a runner.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = DELETE,
    path = "/v0/runners/{runner}/specs/{spec}",
    tags = ["runners"]
}]
pub async fn runner_spec_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerSpecParams>,
) -> Result<ResponseDeleted, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: RunnerSpecParams,
) -> Result<(), HttpError> {
    let query_runner = QueryRunner::from_resource_id(auth_conn!(context), &path_params.runner)?;
    let query_spec = QuerySpec::from_resource_id(auth_conn!(context), &path_params.spec)?;

    let deleted = diesel::delete(
        schema::runner_spec::table
            .filter(schema::runner_spec::runner_id.eq(query_runner.id))
            .filter(schema::runner_spec::spec_id.eq(query_spec.id)),
    )
    .execute(write_conn!(context))
    .map_err(resource_conflict_err!(
        RunnerSpec,
        (&query_runner, &query_spec)
    ))?;

    if deleted == 0 {
        return Err(resource_not_found_err!(
            RunnerSpec,
            (&query_runner, &query_spec)
        )(diesel::result::Error::NotFound));
    }

    Ok(())
}
