use bencher_endpoint::{
    CorsResponse, Endpoint, Get, Patch, Post, ResponseCreated, ResponseOk, TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewRunner, JsonPagination, JsonRunner, JsonRunnerToken, JsonUpdateRunner,
    ResourceName, RunnerResourceId, RunnerSlug, Search, Slug, runner::JsonRunners,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        runner::{InsertRunner, QueryRunner, TokenHash, UpdateRunner},
        user::{admin::AdminUser, auth::BearerToken},
    },
    schema, write_conn,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

/// Runner token prefix
pub const RUNNER_TOKEN_PREFIX: &str = "bencher_runner_";
/// Expected total length of a runner token: prefix (15 chars) + 64 hex chars = 79
pub const RUNNER_TOKEN_LENGTH: usize = RUNNER_TOKEN_PREFIX.len() + 64;

pub type RunnersPagination = JsonPagination<RunnersSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunnersSort {
    /// Sort by runner name.
    #[default]
    Name,
    /// Sort by creation date.
    Created,
}

#[derive(Deserialize, JsonSchema)]
pub struct RunnersQuery {
    /// Filter by runner name, exact match.
    pub name: Option<ResourceName>,
    /// Search by runner name, slug, or UUID.
    pub search: Option<Search>,
    /// Include archived runners.
    #[serde(default)]
    pub archived: bool,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners",
    tags = ["runners"]
}]
pub async fn runners_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<RunnersPagination>,
    _query_params: Query<RunnersQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List runners
///
/// ➕ Bencher Plus: List all runners on the server.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = GET,
    path = "/v0/runners",
    tags = ["runners"]
}]
pub async fn runners_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    pagination_params: Query<RunnersPagination>,
    query_params: Query<RunnersQuery>,
) -> Result<ResponseOk<JsonRunners>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    pagination_params: RunnersPagination,
    query_params: RunnersQuery,
) -> Result<(JsonRunners, TotalCount), HttpError> {
    let runners = get_ls_query(&pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryRunner>(auth_conn!(context))
        .map_err(resource_not_found_err!(Runner))?;

    let json_runners: JsonRunners = {
        let json: Vec<_> = auth_conn!(context, |conn| {
            runners
                .into_iter()
                .map(|r| r.into_json(conn))
                .collect::<Result<Vec<_>, _>>()?
        });
        json.into()
    };

    let total_count = get_ls_query(&pagination_params, &query_params)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(Runner))?
        .try_into()?;

    Ok((json_runners, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &RunnersPagination,
    query_params: &'q RunnersQuery,
) -> schema::runner::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::runner::table.into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::runner::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::runner::name
                .like(search)
                .or(schema::runner::slug.like(search))
                .or(schema::runner::uuid.like(search)),
        );
    }
    if !query_params.archived {
        query = query.filter(schema::runner::archived.is_null());
    }

    match pagination_params.order() {
        RunnersSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::runner::name.asc(), schema::runner::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::runner::name.desc(), schema::runner::slug.desc()))
            },
        },
        RunnersSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::runner::created.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::runner::created.desc()),
        },
    }
}

/// Create a runner
///
/// ➕ Bencher Plus: Create a new runner on the server.
/// The user must be an admin to use this endpoint.
/// Returns the runner token which is only shown once.
#[endpoint {
    method = POST,
    path = "/v0/runners",
    tags = ["runners"]
}]
pub async fn runners_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonNewRunner>,
) -> Result<ResponseCreated<JsonRunnerToken>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    json_runner: JsonNewRunner,
) -> Result<JsonRunnerToken, HttpError> {
    // Generate slug from name if not provided
    let slug = json_runner.slug.unwrap_or_else(|| {
        RunnerSlug::from(
            Slug::new(&json_runner.name).unwrap_or_else(|| Slug::from(uuid::Uuid::new_v4())),
        )
    });

    // Generate random token
    let token = generate_runner_token();
    let token_hash = hash_token(&token);

    let now = context.clock.now();
    let insert_runner = InsertRunner::new(json_runner.name, slug, token_hash, now);
    let uuid = insert_runner.uuid;

    diesel::insert_into(schema::runner::table)
        .values(&insert_runner)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Runner, insert_runner))?;

    // parse() will succeed since token is non-empty
    let secret = token.parse().map_err(|_err| {
        HttpError::for_internal_error("Failed to create runner token".to_owned())
    })?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerCreate);

    Ok(JsonRunnerToken {
        uuid,
        token: secret,
    })
}

#[derive(Deserialize, JsonSchema)]
pub struct RunnerParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}",
    tags = ["runners"]
}]
pub async fn runner_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View a runner
///
/// ➕ Bencher Plus: View a runner on the server.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = GET,
    path = "/v0/runners/{runner}",
    tags = ["runners"]
}]
pub async fn runner_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerParams>,
) -> Result<ResponseOk<JsonRunner>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: RunnerParams,
) -> Result<JsonRunner, HttpError> {
    auth_conn!(context, |conn| {
        let query_runner = QueryRunner::from_resource_id(conn, &path_params.runner)?;
        query_runner.into_json(conn)
    })
}

/// Update a runner
///
/// ➕ Bencher Plus: Update a runner on the server.
/// The user must be an admin to use this endpoint.
/// Can be used to archive or unarchive a runner.
#[endpoint {
    method = PATCH,
    path = "/v0/runners/{runner}",
    tags = ["runners"]
}]
pub async fn runner_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<RunnerParams>,
    body: TypedBody<JsonUpdateRunner>,
) -> Result<ResponseOk<JsonRunner>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: RunnerParams,
    json_runner: JsonUpdateRunner,
) -> Result<JsonRunner, HttpError> {
    let query_runner = QueryRunner::from_resource_id(auth_conn!(context), &path_params.runner)?;

    let now = context.clock.now();
    let update_runner = UpdateRunner::from_json(json_runner.clone(), now);

    diesel::update(schema::runner::table.filter(schema::runner::id.eq(query_runner.id)))
        .set(&update_runner)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(
            Runner,
            (&query_runner, &json_runner)
        ))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerUpdate);

    auth_conn!(context, |conn| {
        let runner = QueryRunner::get(conn, query_runner.id)?;
        runner.into_json(conn)
    })
}

/// Generate a random runner token with prefix
pub fn generate_runner_token() -> String {
    let random_bytes: [u8; 32] = rand::random();
    let encoded = hex::encode(random_bytes);
    format!("{RUNNER_TOKEN_PREFIX}{encoded}")
}

/// Hash a runner token using SHA-256
pub fn hash_token(token: &str) -> TokenHash {
    TokenHash::encode(token)
}
