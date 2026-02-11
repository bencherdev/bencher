use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewSpec, JsonPagination, JsonUpdateSpec, SpecUuid,
    runner::{JsonSpec, JsonSpecs},
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        runner::{InsertSpec, QuerySpec, UpdateSpec},
        user::{admin::AdminUser, auth::BearerToken},
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

pub type SpecsPagination = JsonPagination<SpecsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpecsSort {
    /// Sort by creation date.
    #[default]
    Created,
}

#[derive(Deserialize, JsonSchema)]
pub struct SpecsQuery {
    /// Include archived specs.
    #[serde(default)]
    pub archived: bool,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/specs",
    tags = ["specs"]
}]
pub async fn specs_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<SpecsPagination>,
    _query_params: Query<SpecsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List specs
///
/// List all hardware specs on the server.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = GET,
    path = "/v0/specs",
    tags = ["specs"]
}]
pub async fn specs_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    pagination_params: Query<SpecsPagination>,
    query_params: Query<SpecsQuery>,
) -> Result<ResponseOk<JsonSpecs>, HttpError> {
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
    pagination_params: SpecsPagination,
    query_params: SpecsQuery,
) -> Result<(JsonSpecs, TotalCount), HttpError> {
    let specs = get_ls_query(&pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QuerySpec>(auth_conn!(context))
        .map_err(resource_not_found_err!(Spec))?;

    let json_specs: Vec<JsonSpec> = specs
        .into_iter()
        .map(QuerySpec::into_json)
        .collect::<Result<_, _>>()?;

    let total_count = get_ls_query(&pagination_params, &query_params)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(Spec))?
        .try_into()?;

    Ok((json_specs.into(), total_count))
}

fn get_ls_query<'q>(
    pagination_params: &SpecsPagination,
    query_params: &'q SpecsQuery,
) -> schema::spec::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::spec::table.into_boxed();

    if !query_params.archived {
        query = query.filter(schema::spec::archived.is_null());
    }

    match pagination_params.order() {
        SpecsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::spec::created.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::spec::created.desc()),
        },
    }
}

/// Create a spec
///
/// Create a new hardware spec on the server.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = POST,
    path = "/v0/specs",
    tags = ["specs"]
}]
pub async fn specs_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonNewSpec>,
) -> Result<ResponseCreated<JsonSpec>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(context: &ApiContext, json_spec: JsonNewSpec) -> Result<JsonSpec, HttpError> {
    let insert_spec = InsertSpec::new(&json_spec);
    let uuid = insert_spec.uuid;

    diesel::insert_into(schema::spec::table)
        .values(&insert_spec)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Spec, insert_spec))?;

    let query_spec = QuerySpec::from_uuid(auth_conn!(context), uuid)?;
    query_spec.into_json()
}

#[derive(Deserialize, JsonSchema)]
pub struct SpecParams {
    /// The UUID for a spec.
    pub spec: SpecUuid,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/specs/{spec}",
    tags = ["specs"]
}]
pub async fn spec_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<SpecParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a spec
///
/// View a hardware spec on the server.
/// The user must be an admin to use this endpoint.
#[endpoint {
    method = GET,
    path = "/v0/specs/{spec}",
    tags = ["specs"]
}]
pub async fn spec_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<SpecParams>,
) -> Result<ResponseOk<JsonSpec>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: SpecParams,
) -> Result<JsonSpec, HttpError> {
    let query_spec = QuerySpec::from_uuid(auth_conn!(context), path_params.spec)?;
    query_spec.into_json()
}

/// Update a spec
///
/// Update a hardware spec on the server.
/// The user must be an admin to use this endpoint.
/// Can only be used to archive or unarchive a spec.
#[endpoint {
    method = PATCH,
    path = "/v0/specs/{spec}",
    tags = ["specs"]
}]
pub async fn spec_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<SpecParams>,
    body: TypedBody<JsonUpdateSpec>,
) -> Result<ResponseOk<JsonSpec>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: SpecParams,
    json_spec: JsonUpdateSpec,
) -> Result<JsonSpec, HttpError> {
    let query_spec = QuerySpec::from_uuid(auth_conn!(context), path_params.spec)?;
    let update_spec = UpdateSpec::from(json_spec.clone());

    diesel::update(schema::spec::table.filter(schema::spec::id.eq(query_spec.id)))
        .set(&update_spec)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Spec, (&query_spec, &json_spec)))?;

    let spec = QuerySpec::get(auth_conn!(context), query_spec.id)?;
    spec.into_json()
}

/// Delete a spec
///
/// Delete a hardware spec from the server.
/// The user must be an admin to use this endpoint.
/// Returns 409 Conflict if the spec is referenced by runners or jobs.
#[endpoint {
    method = DELETE,
    path = "/v0/specs/{spec}",
    tags = ["specs"]
}]
pub async fn spec_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<SpecParams>,
) -> Result<ResponseDeleted, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(context: &ApiContext, path_params: SpecParams) -> Result<(), HttpError> {
    let query_spec = QuerySpec::from_uuid(auth_conn!(context), path_params.spec)?;

    diesel::delete(schema::spec::table.filter(schema::spec::id.eq(query_spec.id)))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Spec, query_spec))?;

    Ok(())
}
