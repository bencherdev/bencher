use bencher_json::RunnerResourceId;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_not_found_err},
    model::runner::{QueryRunner, RunnerId},
    schema,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::runners::hash_token;

/// Runner token prefix
const RUNNER_TOKEN_PREFIX: &str = "bencher_runner_";

/// Extract and validate runner token from Authorization header
#[derive(Debug)]
pub struct RunnerToken {
    pub runner_id: RunnerId,
    pub runner_uuid: bencher_json::RunnerUuid,
}

impl RunnerToken {
    pub async fn from_header(
        context: &ApiContext,
        auth_header: Option<&str>,
        expected_runner: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        let token = auth_header
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| forbidden_error("Missing or invalid Authorization header"))?;

        // Validate token format
        if !token.starts_with(RUNNER_TOKEN_PREFIX) {
            return Err(forbidden_error("Invalid runner token format"));
        }

        // Hash the token
        let token_hash = hash_token(token);

        // Look up runner by token hash
        let runner: QueryRunner = schema::runner::table
            .filter(schema::runner::token_hash.eq(&token_hash))
            .filter(schema::runner::archived.is_null())
            .first(auth_conn!(context))
            .optional()
            .map_err(resource_not_found_err!(Runner))?
            .ok_or_else(|| forbidden_error("Invalid runner token"))?;

        // Check if runner is locked
        if runner.is_locked() {
            return Err(forbidden_error("Runner is locked"));
        }

        // Verify the runner matches the path parameter
        let expected = QueryRunner::from_resource_id(auth_conn!(context), expected_runner)?;
        if runner.id != expected.id {
            return Err(forbidden_error(
                "Runner token does not match the runner in the path",
            ));
        }

        Ok(Self {
            runner_id: runner.id,
            runner_uuid: runner.uuid,
        })
    }
}
