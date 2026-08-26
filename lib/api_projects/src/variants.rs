use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    BenchmarkResourceId, JsonDirection, JsonPagination, JsonVariant, JsonVariants, ParameterFilter,
    ParameterSet, ProjectResourceId, ThresholdUuid, VariantUuid,
    project::variant::{JsonNewVariant, JsonUpdateVariant},
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn, auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        conflict_error, resource_conflict_err, resource_not_found_err, with_auth_hint,
        with_token_hint,
    },
    model::{
        project::{
            ProjectId, QueryProject,
            benchmark::QueryBenchmark,
            variant::{QueryVariant, UpdateVariant},
        },
        user::{
            actor::{ApiActor, PubProjectBearerToken},
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn, write_transaction,
};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjVariantsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The slug or UUID for a benchmark.
    pub benchmark: BenchmarkResourceId,
}

pub type ProjVariantsPagination = JsonPagination<ProjVariantsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjVariantsSort {
    /// Sort by variant creation date time.
    #[default]
    Created,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjVariantsQuery {
    /// If set to `true`, only returns archived variants.
    /// If not set or set to `false`, only returns non-archived variants.
    pub archived: Option<bool>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants",
    tags = ["projects", "variants"]
}]
pub async fn proj_variants_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjVariantsParams>,
    _pagination_params: Query<ProjVariantsPagination>,
    _query_params: Query<ProjVariantsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List variants for a benchmark
///
/// List all variants for a benchmark.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
/// By default, the variants are sorted by creation date time.
/// The HTTP response header `X-Total-Count` contains the total number of variants.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants",
    tags = ["projects", "variants"]
}]
pub async fn proj_variants_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjVariantsParams>,
    pagination_params: Query<ProjVariantsPagination>,
    query_params: Query<ProjVariantsQuery>,
) -> Result<ResponseOk<JsonVariants>, HttpError> {
    let api_actor = ApiActor::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok_with_total_count(
        json,
        api_actor.is_auth(),
        total_count,
    ))
}

pub async fn get_ls_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjVariantsParams,
    pagination_params: ProjVariantsPagination,
    query_params: ProjVariantsQuery,
) -> Result<(JsonVariants, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;
    let query_benchmark = QueryBenchmark::from_resource_id(
        actor_conn!(context, api_actor),
        query_project.id,
        &path_params.benchmark,
    )?;

    let variants = get_ls_query(&query_benchmark, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryVariant>(actor_conn!(context, api_actor))
        .map_err(resource_not_found_err!(
            Variant,
            (&query_benchmark, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_variants = variants
        .into_iter()
        .map(|variant| variant.into_json_for_benchmark(&query_benchmark))
        .collect();

    let total_count = get_ls_query(&query_benchmark, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(actor_conn!(context, api_actor))
        .map_err(resource_not_found_err!(
            Variant,
            (&query_benchmark, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_variants, total_count))
}

fn get_ls_query<'q>(
    query_benchmark: &'q QueryBenchmark,
    pagination_params: &ProjVariantsPagination,
    query_params: &'q ProjVariantsQuery,
) -> schema::variant::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = QueryVariant::belonging_to(query_benchmark).into_boxed();

    if let Some(true) = query_params.archived {
        query = query.filter(schema::variant::archived.is_not_null());
    } else {
        query = query.filter(schema::variant::archived.is_null());
    }

    // A variant has no name to break ties with, and `created` is only second
    // granular, so the row `id` orders variants minted within the same second. Without
    // it a page boundary could repeat or skip a variant.
    match pagination_params.order() {
        ProjVariantsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::variant::created.asc(), schema::variant::id.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::variant::created.desc(), schema::variant::id.desc()))
            },
        },
    }
}

/// Create a variant
///
/// Create a variant for a benchmark.
/// The user must have `create` permissions for the project,
/// or provide a valid project key for the project.
/// A variant that already exists for the benchmark is a conflict,
/// so this endpoint never returns an existing variant.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants",
    tags = ["projects", "variants"]
}]
pub async fn proj_variant_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjVariantsParams>,
    body: TypedBody<JsonNewVariant>,
) -> Result<ResponseCreated<JsonVariant>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Post::auth_response_created(json))
}

pub async fn post_inner(
    context: &ApiContext,
    path_params: ProjVariantsParams,
    json_variant: JsonNewVariant,
    api_actor: &ApiActor,
) -> Result<JsonVariant, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Create,
    )?;
    let query_benchmark = QueryBenchmark::from_resource_id(
        auth_conn!(context),
        query_project.id,
        &path_params.benchmark,
    )?;

    let JsonNewVariant { parameters } = json_variant;
    QueryVariant::create(context, query_project.id, query_benchmark.id, &parameters)
        .await
        .map(|variant| variant.into_json_for_benchmark(&query_benchmark))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjVariantParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The slug or UUID for a benchmark.
    pub benchmark: BenchmarkResourceId,
    /// The UUID for a variant.
    pub variant: VariantUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants/{variant}",
    tags = ["projects", "variants"]
}]
pub async fn proj_variant_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjVariantParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a variant
///
/// View a variant for a benchmark.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants/{variant}",
    tags = ["projects", "variants"]
}]
pub async fn proj_variant_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjVariantParams>,
) -> Result<ResponseOk<JsonVariant>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &api_actor)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjVariantParams,
    api_actor: &ApiActor,
) -> Result<JsonVariant, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = QueryProject::is_allowed_actor_pub(
            conn,
            &context.rbac,
            #[cfg(feature = "plus")]
            &context.rate_limiting,
            &path_params.project,
            api_actor,
        )?;
        let query_benchmark =
            QueryBenchmark::from_resource_id(conn, query_project.id, &path_params.benchmark)?;

        QueryVariant::from_uuid(conn, query_benchmark.id, path_params.variant)
            .map(|variant| variant.into_json_for_benchmark(&query_benchmark))
    })
}

/// Update a variant
///
/// Update a variant for a benchmark.
/// The user must have `edit` permissions for the project,
/// or provide a valid project key for the project.
/// Archiving a variant hides it without losing its history,
/// and a report that names the variant again unarchives it.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants/{variant}",
    tags = ["projects", "variants"]
}]
pub async fn proj_variant_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjVariantParams>,
    body: TypedBody<JsonUpdateVariant>,
) -> Result<ResponseOk<JsonVariant>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = patch_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        body.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Patch::auth_response_ok(json))
}

pub async fn patch_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjVariantParams,
    json_variant: JsonUpdateVariant,
) -> Result<JsonVariant, HttpError> {
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Edit,
    )?;
    let query_benchmark = QueryBenchmark::from_resource_id(
        auth_conn!(context),
        query_project.id,
        &path_params.benchmark,
    )?;
    let query_variant =
        QueryVariant::from_uuid(auth_conn!(context), query_benchmark.id, path_params.variant)?;

    let update_variant = UpdateVariant::from(json_variant.clone());
    diesel::update(schema::variant::table.filter(schema::variant::id.eq(query_variant.id)))
        .set(&update_variant)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(
            Variant,
            (&query_variant, &json_variant)
        ))?;

    QueryVariant::get(auth_conn!(context), query_variant.id)
        .map(|variant| variant.into_json_for_benchmark(&query_benchmark))
        .map_err(resource_not_found_err!(Variant, query_variant))
}

/// Delete a variant
///
/// Delete a variant for a benchmark.
/// The user must have `delete` permissions for the project.
/// All reports that use this variant must be deleted first!
/// All thresholds that use this variant must be deleted first!
///
/// A threshold uses a variant when its `parameters` filter names that exact set of
/// parameters. A filter that merely matches the variant, because the variant pins
/// every key the filter names and more besides, is a predicate over values rather
/// than a reference to this row, and it does not stand in the way.
///
/// A benchmark's empty variant cannot be deleted.
/// The empty variant is structural: every benchmark is born with exactly one, and
/// report ingest treats a missing empty variant as data corruption rather than a variant
/// to mint. Deleting it would manufacture exactly that corruption, so the request
/// is refused. Archiving the empty variant stays allowed, because a later report
/// revives it the same way it revives any other archived variant.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/benchmarks/{benchmark}/variants/{variant}",
    tags = ["projects", "variants"]
}]
pub async fn proj_variant_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjVariantParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjVariantParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;
    let query_benchmark = QueryBenchmark::from_resource_id(
        auth_conn!(context),
        query_project.id,
        &path_params.benchmark,
    )?;
    let query_variant =
        QueryVariant::from_uuid(auth_conn!(context), query_benchmark.id, path_params.variant)?;

    if query_variant.parameters.is_empty() {
        return Err(conflict_error(format!(
            "The empty variant ({variant}) for benchmark ({benchmark}) cannot be deleted. Every benchmark must have exactly one empty variant. Archive it or delete the benchmark instead.",
            variant = query_variant.uuid,
            benchmark = query_benchmark.uuid,
        )));
    }

    // The delete goes first and the threshold check goes second, both inside one
    // transaction, so a variant that a report still references is refused for that
    // reason: the foreign key fires on the delete itself and the report refusal is
    // the one the client reads. A variant nothing reports is deleted and then put
    // back if a threshold names it, which is what keeps the two refusals in that
    // order without either of them growing a query the other already does.
    let mut blocking_threshold = None;
    let deleted = write_transaction!(context, |conn| {
        diesel::delete(schema::variant::table.filter(schema::variant::id.eq(query_variant.id)))
            .execute(conn)?;

        blocking_threshold =
            threshold_naming_parameters(conn, query_project.id, &query_variant.parameters)?;
        if blocking_threshold.is_some() {
            return Err(diesel::result::Error::RollbackTransaction);
        }
        diesel::QueryResult::Ok(())
    });

    match (deleted, blocking_threshold) {
        (Ok(()), _) => Ok(()),
        (Err(diesel::result::Error::RollbackTransaction), Some(threshold)) => {
            Err(conflict_error(format!(
                "All thresholds that use this variant must be deleted first! Threshold ({threshold}) checks the variant ({variant}) of benchmark ({benchmark}).",
                variant = query_variant.parameters,
                benchmark = query_benchmark.uuid,
            )))
        },
        (Err(e), _) => Err(resource_conflict_err!(Variant, &query_variant)(e)),
    }
}

/// The first threshold in the project whose filter names this exact set of
/// parameters, if there is one.
///
/// A filter names parameters by canonical equality and only by canonical equality. A
/// filter that merely matches them, say `{"a":1}` against the variant
/// `{"a":1,"b":2}`, is a predicate over values rather than a reference to a row, and
/// deleting the row it happens to match takes nothing out from under it.
///
/// The comparison runs here rather than in SQL because canonical equality is what
/// the canonical form defines and that form is written in Rust. Only the thresholds
/// carrying a filter at all are read, which is a small share of a project's
/// thresholds and empty for every project that has never written one, and deleting a
/// variant is a rare administrative request rather than anything on the ingest
/// path. Nothing here caps the read, so a project that one day holds a great many
/// filtered thresholds is what would move this into SQL.
fn threshold_naming_parameters(
    conn: &mut DbConnection,
    project_id: ProjectId,
    parameters: &ParameterSet,
) -> diesel::QueryResult<Option<ThresholdUuid>> {
    let canonical = parameters.canonical();
    Ok(schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .filter(schema::threshold::parameters.is_not_null())
        .order(schema::threshold::id.asc())
        .select((schema::threshold::uuid, schema::threshold::parameters))
        .load::<(ThresholdUuid, Option<ParameterFilter>)>(conn)?
        .into_iter()
        .find(|(_, parameters)| {
            parameters.as_ref().is_some_and(|parameters| {
                parameters
                    .sets()
                    .iter()
                    .any(|set| set.canonical() == canonical)
            })
        })
        .map(|(uuid, _)| uuid))
}
