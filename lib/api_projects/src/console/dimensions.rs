use std::collections::HashMap;

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BenchmarkName, BenchmarkSlug, BenchmarkUuid, BranchName, BranchSlug, BranchUuid, DateTime,
    GitHash, JsonDirection, MeasureSlug, MeasureUuid, ProjectResourceId, ResourceName, TestbedSlug,
    TestbedUuid,
    project::dimension::{
        ConsoleDimensionsSort, DEFAULT_CONSOLE_DIMENSIONS_PER_PAGE, JsonConsoleBenchmarkRow,
        JsonConsoleBenchmarks, JsonConsoleBranchRow, JsonConsoleBranches,
        JsonConsoleDimensionsQueryParams, JsonConsoleMeasureRow, JsonConsoleMeasures,
        JsonConsoleTestbedRow, JsonConsoleTestbeds,
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            QueryProject, benchmark::BenchmarkId, branch::BranchId, measure::MeasureId,
            metric::MetricId, report::ReportId, testbed::TestbedId,
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
    TextExpressionMethods as _,
    dsl::{count, count_star, exists, max},
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

/// A page of one dimension's rows, filtered, ordered, and limited as the request asks, with the
/// dimension's totals. `$newest` is the dimension's newest report, which last used orders by.
macro_rules! page {
    (
        $conn:ident,
        $from:expr,
        $table:ident,
        $project_id:expr,
        $query_params:expr,
        $newest:expr,
        $not_found:expr
    ) => {{
        let (all, archived) = schema::$table::table
            .filter(schema::$table::project_id.eq($project_id))
            .select((count_star(), count(schema::$table::archived)))
            .get_result::<(i64, i64)>($conn)
            .map_err($not_found)?;
        let request = Request::new($query_params)?;
        let filtered = || {
            let mut query = $from
                .filter(schema::$table::project_id.eq($project_id))
                .into_boxed();
            query = if request.archived {
                query.filter(schema::$table::archived.is_not_null())
            } else {
                query.filter(schema::$table::archived.is_null())
            };
            if let Some(search) = $query_params.search.as_ref() {
                query = query.filter(
                    schema::$table::name
                        .like(search)
                        .or(schema::$table::slug.like(search))
                        .or(schema::$table::uuid.like(search)),
                );
            }
            query
        };
        let total = if $query_params.search.is_some() {
            filtered()
                .count()
                .get_result::<i64>($conn)
                .map_err($not_found)?
        } else if request.archived {
            archived
        } else {
            all - archived
        };
        let mut query = filtered();
        query = match request.sort {
            ConsoleDimensionsSort::Name => query,
            ConsoleDimensionsSort::Created => match request.direction {
                JsonDirection::Asc => query.then_order_by(schema::$table::created.asc()),
                JsonDirection::Desc => query.then_order_by(schema::$table::created.desc()),
            },
            ConsoleDimensionsSort::LastUsed => {
                let query = query.then_order_by($newest.is_null().asc());
                match request.direction {
                    JsonDirection::Asc => query.then_order_by($newest.asc()),
                    JsonDirection::Desc => query.then_order_by($newest.desc()),
                }
            },
        };
        query = match request.direction {
            JsonDirection::Asc => query.then_order_by(schema::$table::name.asc()),
            JsonDirection::Desc => query.then_order_by(schema::$table::name.desc()),
        };
        (
            Totals {
                total: to_u32(total),
                active: to_u32(all - archived),
                archived: to_u32(archived),
            },
            query.offset(request.offset).limit(request.limit),
        )
    }};
}

/// The thresholds on each of `$ids` whose other two dimensions are active, and those that one of
/// them holds archived.
macro_rules! threshold_counts {
    ($conn:ident, $ids:ident, $own:ident, ($a:ident, $a_id:ident), ($b:ident, $b_id:ident)) => {{
        if $ids.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }
        let on = || {
            schema::threshold::table
                .inner_join(schema::$a::table.on(schema::$a::id.eq(schema::threshold::$a_id)))
                .inner_join(schema::$b::table.on(schema::$b::id.eq(schema::threshold::$b_id)))
                .filter(schema::threshold::$own.eq_any($ids))
                .group_by(schema::threshold::$own)
                .select((schema::threshold::$own, count_star()))
        };
        let free = on()
            .filter(
                schema::$a::archived
                    .is_null()
                    .and(schema::$b::archived.is_null()),
            )
            .load($conn);
        let held = on()
            .filter(
                schema::$a::archived
                    .is_not_null()
                    .or(schema::$b::archived.is_not_null()),
            )
            .load($conn);
        free.and_then(|free| Ok((free.into_iter().collect(), held?.into_iter().collect())))
            .map_err(resource_not_found_err!(Threshold, $ids))
    }};
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleDimensionsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/branches",
    tags = ["projects", "branches"],
    unpublished = true,
}]
pub async fn proj_console_branches_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleDimensionsParams>,
    _query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List branches for the console
///
/// List a page of the branches of a project, each with its newest report and its thresholds.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/branches",
    tags = ["projects", "branches"],
    unpublished = true,
}]
pub async fn proj_console_branches_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleDimensionsParams>,
    query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<ResponseOk<JsonConsoleBranches>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = branches_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn branches_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleDimensionsParams,
    query_params: JsonConsoleDimensionsQueryParams,
) -> Result<JsonConsoleBranches, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;
        let not_found = |err| resource_not_found_err!(Branch, (&query_project, &query_params))(err);

        // One seek per head, where a join to the heads would read every report of the branch.
        let newest = schema::head::table
            .filter(schema::head::branch_id.eq(schema::branch::id))
            .select(max(schema::report::table
                .filter(schema::report::head_id.eq(schema::head::id))
                .select(max(schema::report::id))
                .single_value()))
            .single_value();
        let (totals, query) = page!(
            conn,
            schema::branch::table,
            branch,
            query_project.id,
            &query_params,
            newest,
            not_found
        );
        let rows = query
            .select((BranchRow::as_select(), newest))
            .load::<(BranchRow, Option<ReportId>)>(conn)
            .map_err(not_found)?;

        let branch_ids = rows.iter().map(|(branch, _)| branch.id).collect::<Vec<_>>();
        let reports = newest_reports(conn, rows.iter().filter_map(|(_, newest)| *newest))?;
        let start_points = start_points(conn, &branch_ids)?;
        let (thresholds, held_thresholds) = branch_thresholds(conn, &branch_ids)?;

        let branches = rows
            .into_iter()
            .map(|(branch, newest)| {
                let report = newest.and_then(|newest| reports.get(&newest));
                JsonConsoleBranchRow {
                    start_point: start_points.get(&branch.id).cloned(),
                    hash: report.and_then(|report| report.hash.clone()),
                    last_report: report.map(|report| report.created.into()),
                    thresholds: count_of(&thresholds, &branch.id),
                    held_thresholds: count_of(&held_thresholds, &branch.id),
                    uuid: branch.uuid,
                    name: branch.name,
                    slug: branch.slug,
                    created: branch.created.into(),
                    archived: branch.archived.map(Into::into),
                }
            })
            .collect();

        Ok(JsonConsoleBranches {
            total: totals.total,
            active: totals.active,
            archived: totals.archived,
            branches,
        })
    })
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/testbeds",
    tags = ["projects", "testbeds"],
    unpublished = true,
}]
pub async fn proj_console_testbeds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleDimensionsParams>,
    _query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List testbeds for the console
///
/// List a page of the testbeds of a project, each with its spec, its newest report, and its
/// thresholds.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/testbeds",
    tags = ["projects", "testbeds"],
    unpublished = true,
}]
pub async fn proj_console_testbeds_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleDimensionsParams>,
    query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<ResponseOk<JsonConsoleTestbeds>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = testbeds_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn testbeds_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleDimensionsParams,
    query_params: JsonConsoleDimensionsQueryParams,
) -> Result<JsonConsoleTestbeds, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;
        let not_found =
            |err| resource_not_found_err!(Testbed, (&query_project, &query_params))(err);

        // The `(testbed_id, end_time)` index reaches this in one seek, but not the highest ID.
        let newest = schema::report::table
            .filter(schema::report::testbed_id.eq(schema::testbed::id))
            .order((schema::report::end_time.desc(), schema::report::id.desc()))
            .select(schema::report::id)
            .single_value();
        let (totals, query) = page!(
            conn,
            schema::testbed::table.left_join(
                schema::spec::table.on(schema::spec::id.nullable().eq(schema::testbed::spec_id)),
            ),
            testbed,
            query_project.id,
            &query_params,
            newest,
            not_found
        );
        let rows = query
            .select((
                TestbedRow::as_select(),
                schema::spec::name.nullable(),
                newest,
            ))
            .load::<(TestbedRow, Option<ResourceName>, Option<ReportId>)>(conn)
            .map_err(not_found)?;

        let testbed_ids = rows
            .iter()
            .map(|(testbed, ..)| testbed.id)
            .collect::<Vec<_>>();
        let reports = newest_reports(conn, rows.iter().filter_map(|(.., newest)| *newest))?;
        let (thresholds, held_thresholds) = testbed_thresholds(conn, &testbed_ids)?;

        let testbeds = rows
            .into_iter()
            .map(|(testbed, spec, newest)| JsonConsoleTestbedRow {
                spec,
                last_report: newest
                    .and_then(|newest| reports.get(&newest))
                    .map(|report| report.created.into()),
                thresholds: count_of(&thresholds, &testbed.id),
                held_thresholds: count_of(&held_thresholds, &testbed.id),
                uuid: testbed.uuid,
                name: testbed.name,
                slug: testbed.slug,
                created: testbed.created.into(),
                archived: testbed.archived.map(Into::into),
            })
            .collect();

        Ok(JsonConsoleTestbeds {
            total: totals.total,
            active: totals.active,
            archived: totals.archived,
            testbeds,
        })
    })
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/benchmarks",
    tags = ["projects", "benchmarks"],
    unpublished = true,
}]
pub async fn proj_console_benchmarks_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleDimensionsParams>,
    _query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List benchmarks for the console
///
/// List a page of the benchmarks of a project, each with its active variants and its newest
/// report.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/benchmarks",
    tags = ["projects", "benchmarks"],
    unpublished = true,
}]
pub async fn proj_console_benchmarks_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleDimensionsParams>,
    query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<ResponseOk<JsonConsoleBenchmarks>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = benchmarks_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn benchmarks_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleDimensionsParams,
    query_params: JsonConsoleDimensionsQueryParams,
) -> Result<JsonConsoleBenchmarks, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;
        let not_found =
            |err| resource_not_found_err!(Benchmark, (&query_project, &query_params))(err);

        let newest = schema::report_benchmark::table
            .filter(schema::report_benchmark::benchmark_id.eq(schema::benchmark::id))
            .select(max(schema::report_benchmark::report_id))
            .single_value();
        let (totals, query) = page!(
            conn,
            schema::benchmark::table,
            benchmark,
            query_project.id,
            &query_params,
            newest,
            not_found
        );
        let rows = query
            .select((BenchmarkRow::as_select(), newest))
            .load::<(BenchmarkRow, Option<ReportId>)>(conn)
            .map_err(not_found)?;

        let benchmark_ids = rows
            .iter()
            .map(|(benchmark, _)| benchmark.id)
            .collect::<Vec<_>>();
        let reports = newest_reports(conn, rows.iter().filter_map(|(_, newest)| *newest))?;
        let variants = if benchmark_ids.is_empty() {
            HashMap::new()
        } else {
            schema::variant::table
                .filter(schema::variant::benchmark_id.eq_any(&benchmark_ids))
                .filter(schema::variant::archived.is_null())
                .filter(exists(schema::report_benchmark::table.filter(
                    schema::report_benchmark::variant_id.eq(schema::variant::id),
                )))
                .group_by(schema::variant::benchmark_id)
                .select((schema::variant::benchmark_id, count_star()))
                .load::<(BenchmarkId, i64)>(conn)
                .map(|counts| counts.into_iter().collect())
                .map_err(resource_not_found_err!(Variant, &benchmark_ids))?
        };

        let benchmarks = rows
            .into_iter()
            .map(|(benchmark, newest)| JsonConsoleBenchmarkRow {
                variants: count_of(&variants, &benchmark.id),
                last_report: newest
                    .and_then(|newest| reports.get(&newest))
                    .map(|report| report.created.into()),
                uuid: benchmark.uuid,
                name: benchmark.name,
                slug: benchmark.slug,
                created: benchmark.created.into(),
                archived: benchmark.archived.map(Into::into),
            })
            .collect();

        Ok(JsonConsoleBenchmarks {
            total: totals.total,
            active: totals.active,
            archived: totals.archived,
            benchmarks,
        })
    })
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/measures",
    tags = ["projects", "measures"],
    unpublished = true,
}]
pub async fn proj_console_measures_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleDimensionsParams>,
    _query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List measures for the console
///
/// List a page of the measures of a project, each with its newest report and its thresholds.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/measures",
    tags = ["projects", "measures"],
    unpublished = true,
}]
pub async fn proj_console_measures_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleDimensionsParams>,
    query_params: Query<JsonConsoleDimensionsQueryParams>,
) -> Result<ResponseOk<JsonConsoleMeasures>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = measures_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn measures_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleDimensionsParams,
    query_params: JsonConsoleDimensionsQueryParams,
) -> Result<JsonConsoleMeasures, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;
        let not_found =
            |err| resource_not_found_err!(Measure, (&query_project, &query_params))(err);

        // A measure's values are stored after the report that carries them, so its newest value
        // finds its newest report through one index seek.
        let newest = schema::metric::table
            .filter(schema::metric::measure_id.eq(schema::measure::id))
            .select(max(schema::metric::id))
            .single_value();
        let (totals, query) = page!(
            conn,
            schema::measure::table,
            measure,
            query_project.id,
            &query_params,
            newest,
            not_found
        );
        let rows = query
            .select((MeasureRow::as_select(), newest))
            .load::<(MeasureRow, Option<MetricId>)>(conn)
            .map_err(not_found)?;

        let measure_ids = rows
            .iter()
            .map(|(measure, _)| measure.id)
            .collect::<Vec<_>>();
        let metric_ids = rows
            .iter()
            .filter_map(|(_, newest)| *newest)
            .collect::<Vec<_>>();
        let reports = if metric_ids.is_empty() {
            HashMap::new()
        } else {
            schema::metric::table
                .inner_join(
                    schema::report_benchmark::table
                        .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
                )
                .inner_join(
                    schema::report::table
                        .on(schema::report::id.eq(schema::report_benchmark::report_id)),
                )
                .filter(schema::metric::id.eq_any(&metric_ids))
                .select((schema::metric::id, schema::report::created))
                .load::<(MetricId, DateTime)>(conn)
                .map(|reports| reports.into_iter().collect::<HashMap<_, _>>())
                .map_err(resource_not_found_err!(Report, &metric_ids))?
        };
        let (thresholds, held_thresholds) = measure_thresholds(conn, &measure_ids)?;

        let measures = rows
            .into_iter()
            .map(|(measure, newest)| JsonConsoleMeasureRow {
                last_report: newest
                    .and_then(|newest| reports.get(&newest))
                    .map(|&created| created.into()),
                thresholds: count_of(&thresholds, &measure.id),
                held_thresholds: count_of(&held_thresholds, &measure.id),
                uuid: measure.uuid,
                name: measure.name,
                slug: measure.slug,
                units: measure.units,
                created: measure.created.into(),
                archived: measure.archived.map(Into::into),
            })
            .collect();

        Ok(JsonConsoleMeasures {
            total: totals.total,
            active: totals.active,
            archived: totals.archived,
            measures,
        })
    })
}

/// The project when the actor may read it, which takes a login even on a public project.
fn readable_project(
    conn: &mut DbConnection,
    context: &ApiContext,
    project: &ProjectResourceId,
    api_actor: &ApiActor,
) -> Result<QueryProject, HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        conn,
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        project,
        api_actor,
    )?;
    if api_actor.is_auth() {
        Ok(query_project)
    } else {
        Err(query_project
            .auth_state(api_actor)
            .auth_error(project, Permission::View))
    }
}

/// What a list request asks for, with its defaults filled in.
struct Request {
    archived: bool,
    sort: ConsoleDimensionsSort,
    direction: JsonDirection,
    offset: i64,
    limit: i64,
}

impl Request {
    fn new(query_params: &JsonConsoleDimensionsQueryParams) -> Result<Self, HttpError> {
        let sort = query_params.sort.unwrap_or_default();
        let direction = query_params.direction.unwrap_or(match sort {
            ConsoleDimensionsSort::Name => JsonDirection::Asc,
            ConsoleDimensionsSort::Created | ConsoleDimensionsSort::LastUsed => JsonDirection::Desc,
        });
        let per_page = query_params
            .per_page
            .unwrap_or(DEFAULT_CONSOLE_DIMENSIONS_PER_PAGE);
        let offset = match (query_params.page, query_params.offset) {
            (Some(_), Some(_)) => {
                return Err(bad_request_error(
                    "Ask for either a page or an offset, not both",
                ));
            },
            (None, Some(offset)) => i64::from(offset),
            (page, None) => i64::from(page.unwrap_or(1).saturating_sub(1)) * i64::from(per_page),
        };
        Ok(Self {
            archived: query_params.archived.unwrap_or_default(),
            sort,
            direction,
            offset,
            limit: i64::from(per_page),
        })
    }
}

struct Totals {
    total: u32,
    active: u32,
    archived: u32,
}

struct NewestReport {
    created: DateTime,
    hash: Option<GitHash>,
}

fn newest_reports(
    conn: &mut DbConnection,
    report_ids: impl Iterator<Item = ReportId>,
) -> Result<HashMap<ReportId, NewestReport>, HttpError> {
    let report_ids = report_ids.collect::<Vec<_>>();
    if report_ids.is_empty() {
        return Ok(HashMap::new());
    }
    schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(schema::report::id.eq_any(&report_ids))
        .select((
            schema::report::id,
            schema::report::created,
            schema::version::hash,
        ))
        .load::<(ReportId, DateTime, Option<GitHash>)>(conn)
        .map(|reports| {
            reports
                .into_iter()
                .map(|(id, created, hash)| (id, NewestReport { created, hash }))
                .collect()
        })
        .map_err(resource_not_found_err!(Report, &report_ids))
}

diesel::alias!(
    schema::head as start_head: StartHead,
    schema::branch as start_branch: StartBranch,
);

/// The name of the branch that each branch's current head started from, for those that have one.
fn start_points(
    conn: &mut DbConnection,
    branch_ids: &[BranchId],
) -> Result<HashMap<BranchId, BranchName>, HttpError> {
    if branch_ids.is_empty() {
        return Ok(HashMap::new());
    }
    schema::branch::table
        .inner_join(schema::head::table.on(schema::branch::head_id.eq(schema::head::id.nullable())))
        .inner_join(
            schema::head_version::table
                .on(schema::head::start_point_id.eq(schema::head_version::id.nullable())),
        )
        .inner_join(
            start_head.on(start_head
                .field(schema::head::id)
                .eq(schema::head_version::head_id)),
        )
        .inner_join(
            start_branch.on(start_branch
                .field(schema::branch::id)
                .eq(start_head.field(schema::head::branch_id))),
        )
        .filter(schema::branch::id.eq_any(branch_ids))
        .select((schema::branch::id, start_branch.field(schema::branch::name)))
        .load::<(BranchId, BranchName)>(conn)
        .map(|start_points| start_points.into_iter().collect())
        .map_err(resource_not_found_err!(Branch, branch_ids))
}

type Counts<K> = HashMap<K, i64>;

fn branch_thresholds(
    conn: &mut DbConnection,
    branch_ids: &[BranchId],
) -> Result<(Counts<BranchId>, Counts<BranchId>), HttpError> {
    threshold_counts!(
        conn,
        branch_ids,
        branch_id,
        (testbed, testbed_id),
        (measure, measure_id)
    )
}

fn testbed_thresholds(
    conn: &mut DbConnection,
    testbed_ids: &[TestbedId],
) -> Result<(Counts<TestbedId>, Counts<TestbedId>), HttpError> {
    threshold_counts!(
        conn,
        testbed_ids,
        testbed_id,
        (branch, branch_id),
        (measure, measure_id)
    )
}

fn measure_thresholds(
    conn: &mut DbConnection,
    measure_ids: &[MeasureId],
) -> Result<(Counts<MeasureId>, Counts<MeasureId>), HttpError> {
    threshold_counts!(
        conn,
        measure_ids,
        measure_id,
        (branch, branch_id),
        (testbed, testbed_id)
    )
}

fn count_of<K: std::hash::Hash + Eq>(counts: &Counts<K>, key: &K) -> u32 {
    counts.get(key).map_or(0, |&count| to_u32(count))
}

fn to_u32(count: i64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::branch)]
struct BranchRow {
    id: BranchId,
    uuid: BranchUuid,
    name: BranchName,
    slug: BranchSlug,
    created: DateTime,
    archived: Option<DateTime>,
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::testbed)]
struct TestbedRow {
    id: TestbedId,
    uuid: TestbedUuid,
    name: ResourceName,
    slug: TestbedSlug,
    created: DateTime,
    archived: Option<DateTime>,
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::benchmark)]
struct BenchmarkRow {
    id: BenchmarkId,
    uuid: BenchmarkUuid,
    name: BenchmarkName,
    slug: BenchmarkSlug,
    created: DateTime,
    archived: Option<DateTime>,
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::measure)]
struct MeasureRow {
    id: MeasureId,
    uuid: MeasureUuid,
    name: ResourceName,
    slug: MeasureSlug,
    units: ResourceName,
    created: DateTime,
    archived: Option<DateTime>,
}
