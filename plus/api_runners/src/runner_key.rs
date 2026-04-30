use bencher_json::{RunnerKey, RunnerResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_not_found_err, unauthorized_error},
    model::runner::{QueryRunner, RunnerId},
    schema,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, RequestContext};

use crate::runners::hash_key;

/// Authenticated runner identity, extracted from a valid runner key
#[derive(Debug)]
pub struct RunnerAuth {
    pub runner_id: RunnerId,
    pub runner_uuid: bencher_json::RunnerUuid,
}

impl RunnerAuth {
    /// Extract and validate runner key from a request.
    pub async fn from_request(
        rqctx: &RequestContext<ApiContext>,
        expected_runner: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        let auth_header = rqctx
            .request
            .headers()
            .get(bencher_json::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        Self::from_header(rqctx.context(), auth_header, expected_runner).await
    }

    async fn from_header(
        context: &ApiContext,
        auth_header: Option<&str>,
        expected_runner: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        let key_str = auth_header
            .and_then(bencher_json::strip_bearer_token)
            .ok_or_else(|| unauthorized_error("Missing or invalid Authorization header"))?;

        let runner_key: RunnerKey = key_str
            .parse()
            .map_err(|_err| unauthorized_error("Invalid runner key format"))?;

        let key_hash = hash_key(&runner_key);

        // Look up runner by key hash AND path parameter in a single query
        let mut query = schema::runner::table
            .filter(schema::runner::key_hash.eq(&key_hash))
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
            .ok_or_else(|| unauthorized_error("Invalid runner key"))?;

        Ok(Self {
            runner_id: runner.id,
            runner_uuid: runner.uuid,
        })
    }
}
