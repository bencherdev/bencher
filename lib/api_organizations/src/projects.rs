use bencher_endpoint::{
    CorsResponse, Endpoint, Get, Post, ResponseCreated, ResponseOk, TotalCount,
};
use bencher_json::{
    project::measure::built_in::default::{Latency, Throughput},
    JsonDirection, JsonNewProject, JsonPagination, JsonProject, JsonProjects, ResourceId,
    ResourceName, Search,
};
use bencher_rbac::organization::Permission;
#[cfg(feature = "plus")]
use bencher_schema::model::organization::plan::PlanKind;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        organization::QueryOrganization,
        project::{
            branch::InsertBranch,
            measure::{InsertMeasure, QueryMeasure},
            testbed::{InsertTestbed, QueryTestbed},
            threshold::InsertThreshold,
            InsertProject, QueryProject,
        },
        user::auth::{AuthUser, BearerToken},
    },
    schema,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

#[derive(Deserialize, JsonSchema)]
pub struct OrgProjectsParams {
    /// The slug or UUID for an organization.
    pub organization: ResourceId,
}

pub type OrgProjectsPagination = JsonPagination<OrgProjectsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgProjectsSort {
    /// Sort by project name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrgProjectsQuery {
    /// Filter by project name, exact match.
    pub name: Option<ResourceName>,
    /// Search by project name, slug, or UUID.
    pub search: Option<Search>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_projects_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgProjectsParams>,
    _pagination_params: Query<OrgProjectsPagination>,
    _query_params: Query<OrgProjectsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List organization projects
///
/// List projects for an organization.
/// The user must have `view` permissions for the organization.
/// By default, the projects are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of organization projects.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_projects_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgProjectsParams>,
    pagination_params: Query<OrgProjectsPagination>,
    query_params: Query<OrgProjectsQuery>,
) -> Result<ResponseOk<JsonProjects>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: OrgProjectsParams,
    pagination_params: OrgProjectsPagination,
    query_params: OrgProjectsQuery,
    auth_user: &AuthUser,
) -> Result<(JsonProjects, TotalCount), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;

    let projects = get_ls_query(&query_organization, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryProject>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Project,
            (&query_organization, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_projects = conn_lock!(context, |conn| projects
        .into_iter()
        .map(|project| project.into_json_for_organization(conn, &query_organization))
        .collect());

    let total_count = get_ls_query(&query_organization, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Project,
            (&query_organization, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_projects, total_count))
}

fn get_ls_query<'q>(
    query_organization: &'q QueryOrganization,
    pagination_params: &OrgProjectsPagination,
    query_params: &'q OrgProjectsQuery,
) -> schema::project::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = QueryProject::belonging_to(query_organization).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::project::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::project::name
                .like(search)
                .or(schema::project::slug.like(search))
                .or(schema::project::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        OrgProjectsSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::project::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::project::name.desc()),
        },
    }
}

/// Create a project for an organization
///
/// Create a new project for an organization.
/// The user must have `create` permissions for the organization.
/// The new project will have a `main` branch, a `localhost` testbed, `latency` and `throughput` measures, and a threshold for both measures.
/// ➕ Bencher Plus: The project visibility must be `public` unless the organization has a valid Bencher Plus subscription.
#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn org_project_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgProjectsParams>,
    body: TypedBody<JsonNewProject>,
) -> Result<ResponseCreated<JsonProject>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: OrgProjectsParams,
    json_project: JsonNewProject,
    auth_user: &AuthUser,
) -> Result<JsonProject, HttpError> {
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    #[cfg(feature = "plus")]
    InsertProject::rate_limit(context, &query_organization).await?;

    if let Some(visibility) = json_project.visibility {
        // Check project visibility
        #[cfg(not(feature = "plus"))]
        QueryProject::is_visibility_public(visibility)?;
        #[cfg(feature = "plus")]
        PlanKind::check_for_organization(
            context,
            context.biller.as_ref(),
            &context.licensor,
            &query_organization,
            visibility,
        )
        .await?;
    }

    let insert_project =
        InsertProject::from_json(conn_lock!(context), &query_organization, json_project);
    // Create a new project
    let query_project =
        QueryProject::create(log, context, auth_user, &query_organization, insert_project).await?;

    // Add a `main` branch to the project
    let query_branch = InsertBranch::main(log, context, query_project.id).await?;
    slog::debug!(log, "Added project branch: {query_branch:?}");
    let branch_id = query_branch.id;

    // Add a `localhost` testbed to the project
    let insert_testbed = InsertTestbed::localhost(conn_lock!(context), query_project.id);
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Testbed, insert_testbed))?;
    let testbed_id = QueryTestbed::get_id(conn_lock!(context), insert_testbed.uuid)?;
    slog::debug!(log, "Added project testbed: {insert_testbed:?}");

    // Add a `latency` measure to the project
    let insert_measure =
        InsertMeasure::from_measure::<Latency>(conn_lock!(context), query_project.id);
    diesel::insert_into(schema::measure::table)
        .values(&insert_measure)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Measure, insert_measure))?;
    let measure_id = QueryMeasure::get_id(conn_lock!(context), insert_measure.uuid)?;
    slog::debug!(log, "Added project measure: {insert_measure:?}");
    // Add a `latency` threshold to the project
    let threshold_id = InsertThreshold::upper_boundary(
        conn_lock!(context),
        query_project.id,
        branch_id,
        testbed_id,
        measure_id,
    )?;
    slog::debug!(log, "Added project threshold: {threshold_id}");

    // Add a `throughput` measure to the project
    let insert_measure =
        InsertMeasure::from_measure::<Throughput>(conn_lock!(context), query_project.id);
    diesel::insert_into(schema::measure::table)
        .values(&insert_measure)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Measure, insert_measure))?;
    let measure_id = QueryMeasure::get_id(conn_lock!(context), insert_measure.uuid)?;
    slog::debug!(log, "Added project measure: {insert_measure:?}");
    // Add a `throughput` threshold to the project
    let threshold_id = InsertThreshold::lower_boundary(
        conn_lock!(context),
        query_project.id,
        branch_id,
        testbed_id,
        measure_id,
    )?;
    slog::debug!(log, "Added project threshold: {threshold_id}");

    query_project.into_json(conn_lock!(context))
}
