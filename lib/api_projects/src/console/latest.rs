use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BenchmarkUuid, BranchName, BranchSlug, BranchUuid, DateTime, GitHash, HeadUuid, MeasureSlug,
    MeasureUuid, ProjectResourceId, ReportUuid, ResourceName, TestbedSlug, TestbedUuid,
    project::{
        console::{
            JsonConsoleBranch, JsonConsoleLatestReport, JsonConsoleMeasure, JsonConsoleTestbed,
        },
        head::{JsonVersion, VersionNumber},
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{resource_not_found_err, with_auth_hint},
    model::{
        project::{QueryProject, benchmark::BenchmarkId, measure::MeasureId, report::ReportId},
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleLatestParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a benchmark.
    pub benchmark: BenchmarkUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/benchmarks/{benchmark}/latest",
    tags = ["projects", "benchmarks"],
    unpublished = true,
}]
pub async fn proj_console_latest_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleLatestParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a benchmark's newest report for the console
///
/// The branch, testbed, and version of the newest report that has the benchmark,
/// and the measures the benchmark reported in it.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/benchmarks/{benchmark}/latest",
    tags = ["projects", "benchmarks"],
    unpublished = true,
}]
pub async fn proj_console_latest_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleLatestParams>,
) -> Result<ResponseOk<JsonConsoleLatestReport>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &api_actor)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjConsoleLatestParams,
    api_actor: &ApiActor,
) -> Result<JsonConsoleLatestReport, HttpError> {
    let ProjConsoleLatestParams { project, benchmark } = path_params;
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &project,
        api_actor,
    )?;
    // Only the public plot is served without a login, even for a public project.
    if !api_actor.is_auth() {
        return Err(query_project
            .auth_state(api_actor)
            .auth_error(&project, Permission::View));
    }
    let conn = actor_conn!(context, api_actor);

    let benchmark_id = schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .filter(schema::benchmark::uuid.eq(benchmark))
        .select(schema::benchmark::id)
        .first::<BenchmarkId>(conn)
        .map_err(resource_not_found_err!(
            Benchmark,
            (&query_project, benchmark)
        ))?;
    // The newest report the API took, read off the benchmark's own index.
    let report_id = schema::report_benchmark::table
        .filter(schema::report_benchmark::benchmark_id.eq(benchmark_id))
        .order(schema::report_benchmark::report_id.desc())
        .select(schema::report_benchmark::report_id)
        .first::<ReportId>(conn)
        .map_err(resource_not_found_err!(Report, (&query_project, benchmark)))?;
    let measures = measures(conn, report_id, benchmark_id).map_err(resource_not_found_err!(
        Measure,
        (&query_project, benchmark)
    ))?;
    let (uuid, start_time, number, hash, branch, testbed) = latest_row(conn, report_id)
        .map_err(resource_not_found_err!(Report, (&query_project, benchmark)))?;

    let (branch_uuid, branch_name, branch_slug, head) = branch;
    let (testbed_uuid, testbed_name, testbed_slug) = testbed;
    Ok(JsonConsoleLatestReport {
        uuid,
        branch: JsonConsoleBranch {
            uuid: branch_uuid,
            name: branch_name,
            slug: branch_slug,
            head,
        },
        testbed: JsonConsoleTestbed {
            uuid: testbed_uuid,
            name: testbed_name,
            slug: testbed_slug,
            #[cfg(feature = "plus")]
            spec: None,
        },
        version: JsonVersion { number, hash },
        start_time: start_time.into(),
        measures,
    })
}

fn latest_row(conn: &mut DbConnection, report_id: ReportId) -> diesel::QueryResult<LatestRow> {
    schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .filter(schema::report::id.eq(report_id))
        .select((
            schema::report::uuid,
            schema::report::start_time,
            schema::version::number,
            schema::version::hash,
            (
                schema::branch::uuid,
                schema::branch::name,
                schema::branch::slug,
                schema::head::uuid,
            ),
            (
                schema::testbed::uuid,
                schema::testbed::name,
                schema::testbed::slug,
            ),
        ))
        .first::<LatestRow>(conn)
}

type LatestRow = (
    ReportUuid,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    (BranchUuid, BranchName, BranchSlug, HeadUuid),
    (TestbedUuid, ResourceName, TestbedSlug),
);

/// The measures the benchmark reported in the report, in name order.
fn measures(
    conn: &mut DbConnection,
    report_id: ReportId,
    benchmark_id: BenchmarkId,
) -> diesel::QueryResult<Vec<JsonConsoleMeasure>> {
    let measure_ids = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .filter(schema::report_benchmark::benchmark_id.eq(benchmark_id))
        .select(schema::metric::measure_id)
        .distinct()
        .load::<MeasureId>(conn)?;
    Ok(schema::measure::table
        .filter(schema::measure::id.eq_any(measure_ids))
        .order(schema::measure::name)
        .select((
            schema::measure::uuid,
            schema::measure::name,
            schema::measure::slug,
            schema::measure::units,
        ))
        .load::<(MeasureUuid, ResourceName, MeasureSlug, ResourceName)>(conn)?
        .into_iter()
        .map(|(uuid, name, slug, units)| JsonConsoleMeasure {
            uuid,
            name,
            slug,
            units,
        })
        .collect())
}
