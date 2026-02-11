use bencher_json::RunnerResourceId;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_not_found_err},
    model::runner::{QueryRunner, RunnerId},
    schema,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, RequestContext};

use crate::runners::{RUNNER_TOKEN_LENGTH, RUNNER_TOKEN_PREFIX, hash_token};

/// Extract and validate runner token from Authorization header
#[derive(Debug)]
pub struct RunnerToken {
    pub runner_id: RunnerId,
    pub runner_uuid: bencher_json::RunnerUuid,
}

impl RunnerToken {
    /// Extract and validate runner token from a request.
    pub async fn from_request(
        rqctx: &RequestContext<ApiContext>,
        expected_runner: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        let auth_header = rqctx
            .request
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok());
        Self::from_header(rqctx.context(), auth_header, expected_runner).await
    }

    async fn from_header(
        context: &ApiContext,
        auth_header: Option<&str>,
        expected_runner: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        let token = auth_header
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| forbidden_error("Missing or invalid Authorization header"))?;

        // Validate token format (prefix + length)
        if !token.starts_with(RUNNER_TOKEN_PREFIX) || token.len() != RUNNER_TOKEN_LENGTH {
            return Err(forbidden_error("Invalid runner token format"));
        }

        // Hash the token
        let token_hash = hash_token(token);

        // Look up runner by token hash AND path parameter in a single query
        let mut query = schema::runner::table
            .filter(schema::runner::token_hash.eq(&token_hash))
            .filter(schema::runner::archived.is_null())
            .into_boxed();

        match expected_runner {
            RunnerResourceId::Uuid(uuid) => {
                query = query.filter(schema::runner::uuid.eq(*uuid));
            },
            RunnerResourceId::Slug(slug) => {
                query = query.filter(schema::runner::slug.eq(AsRef::<str>::as_ref(slug)));
            },
        }

        let runner: QueryRunner = query
            .first(auth_conn!(context))
            .optional()
            .map_err(resource_not_found_err!(Runner))?
            .ok_or_else(|| forbidden_error("Invalid runner token"))?;

        Ok(Self {
            runner_id: runner.id,
            runner_uuid: runner.uuid,
        })
    }
}
