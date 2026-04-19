use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{DateTime, JsonRunnerKey, RunnerResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{conflict_error, resource_conflict_err},
    model::{
        runner::{QueryRunner, UpdateRunner},
        user::{admin::AdminUser, auth::BearerToken},
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::runners::{generate_runner_key, hash_key};

#[derive(Deserialize, JsonSchema)]
pub struct RunnerKeyParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}/key",
    tags = ["runners"]
}]
pub async fn runner_key_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerKeyParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Rotate runner key
///
/// ➕ Bencher Plus: Generate a new key for a runner, invalidating the old one.
/// The user must be an admin to use this endpoint.
/// Returns the new key which is only shown once.
#[endpoint {
    method = POST,
    path = "/v0/runners/{runner}/key",
    tags = ["runners"]
}]
pub async fn runner_key_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerKeyParams>,
) -> Result<ResponseCreated<JsonRunnerKey>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: RunnerKeyParams,
) -> Result<JsonRunnerKey, HttpError> {
    let query_runner = QueryRunner::from_resource_id(auth_conn!(context), &path_params.runner)?;

    if query_runner.is_archived() {
        return Err(conflict_error("Cannot rotate key for an archived runner"));
    }

    // Generate new key
    let key = generate_runner_key();
    let key_hash = hash_key(&key);

    let update_runner = UpdateRunner {
        key_hash: Some(key_hash),
        modified: Some(DateTime::now()),
        ..Default::default()
    };

    diesel::update(schema::runner::table.filter(schema::runner::id.eq(query_runner.id)))
        .set(&update_runner)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Runner, query_runner))?;

    // parse() will succeed since key is non-empty
    let secret = key
        .parse()
        .map_err(|_err| HttpError::for_internal_error("Failed to create runner key".to_owned()))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerKeyRotate);

    Ok(JsonRunnerKey {
        uuid: query_runner.uuid,
        key: secret,
    })
}
