use std::{collections::HashMap, time::Duration};

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, JsonPerfQuery, MeasureUuid, ProjectResourceId,
    TestbedUuid,
    project::console::{
        DEFAULT_CONSOLE_PLOTS_PER_PAGE, JsonConsolePerfQuery, JsonConsolePlot,
        JsonConsolePlotTitle, JsonConsolePlots, JsonConsolePlotsQueryParams, JsonConsoleWindow,
    },
};
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            QueryProject,
            plot::{PlotId, QueryPlot},
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    before,
    perf::{Window, draw},
    thresholds::readable_project,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsolePlotsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/plots",
    tags = ["projects", "plots"],
    unpublished = true,
}]
pub async fn proj_console_plots_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsolePlotsParams>,
    _query_params: Query<JsonConsolePlotsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List pinned plots for the console
///
/// A page of the project's pinned plots, top first, each with the lines it draws,
/// and the order of every pinned plot.
/// Each plot draws its own window ending now, unless the request names the page's window.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/plots",
    tags = ["projects", "plots"],
    unpublished = true,
}]
pub async fn proj_console_plots_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsolePlotsParams>,
    query_params: Query<JsonConsolePlotsQueryParams>,
) -> Result<ResponseOk<JsonConsolePlots>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjConsolePlotsParams,
    query_params: JsonConsolePlotsQueryParams,
    api_actor: &ApiActor,
) -> Result<JsonConsolePlots, HttpError> {
    let JsonConsolePlotsQueryParams {
        page,
        per_page,
        start_time,
        end_time,
    } = query_params;
    let end_time = end_time.map_or_else(|| context.clock.now(), DateTime::from);
    let start_time = start_time.map(DateTime::from);
    if let Some(start_time) = start_time
        && start_time.timestamp() > end_time.timestamp()
    {
        return Err(bad_request_error(format!(
            "The start time ({start_time}) is after the end time ({end_time})"
        )));
    }
    actor_conn!(context, api_actor, |conn| {
        let project = readable_project(conn, context, &path_params.project, api_actor)?;
        let plots = QueryPlot::belonging_to(&project)
            .order(schema::plot::rank.asc())
            .load::<QueryPlot>(conn)
            .map_err(resource_not_found_err!(Plot, &project))?;
        let order = plots
            .iter()
            .map(|plot| JsonConsolePlotTitle {
                uuid: plot.uuid,
                title: plot.title.clone(),
            })
            .collect();

        let per_page = usize::from(per_page.unwrap_or(DEFAULT_CONSOLE_PLOTS_PER_PAGE));
        let skipped = usize::try_from(page.unwrap_or(1).saturating_sub(1))
            .unwrap_or(usize::MAX)
            .saturating_mul(per_page);
        let shown = plots
            .into_iter()
            .skip(skipped)
            .take(per_page)
            .collect::<Vec<_>>();
        let mut dimensions = Dimensions::of(conn, &project, &shown)?;
        let plots = shown
            .into_iter()
            .map(|plot| {
                let start_time = start_time.unwrap_or_else(|| {
                    let length = Duration::from_secs(u64::from(u32::from(plot.window)));
                    before(end_time, length).unwrap_or(end_time)
                });
                drawn_plot(
                    conn,
                    &project,
                    plot,
                    &mut dimensions,
                    (start_time, end_time),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(JsonConsolePlots { order, plots })
    })
}

/// A plot as saved, drawn from `start_time` to `end_time` with the plot query's own code.
fn drawn_plot(
    conn: &mut DbConnection,
    project: &QueryProject,
    plot: QueryPlot,
    dimensions: &mut Dimensions,
    (start_time, end_time): (DateTime, DateTime),
) -> Result<JsonConsolePlot, HttpError> {
    let branches = dimensions.branches.remove(&plot.id).unwrap_or_default();
    let testbeds = dimensions.testbeds.remove(&plot.id).unwrap_or_default();
    let benchmarks = dimensions.benchmarks.remove(&plot.id).unwrap_or_default();
    let measures = dimensions.measures.remove(&plot.id).unwrap_or_default();
    let query = JsonConsolePerfQuery {
        perf: JsonPerfQuery {
            heads: vec![None; branches.len()],
            branches: branches.clone(),
            #[cfg(feature = "plus")]
            specs: vec![None; testbeds.len()],
            testbeds: testbeds.clone(),
            benchmarks: benchmarks.clone(),
            parameters: plot
                .parameters
                .as_ref()
                .filter(|filter| !filter.is_match_all())
                .map(|filter| filter.sets().to_vec()),
            measures: measures.clone(),
            start_time: None,
            end_time: None,
        },
        metrics: plot
            .metrics
            .as_ref()
            .filter(|metrics| !metrics.is_empty())
            .map(|metrics| metrics.names().to_vec()),
    };
    let echoed = JsonConsoleWindow {
        start_time: start_time.into(),
        end_time: end_time.into(),
        clamped: false,
    };
    let window = Window::new(start_time, end_time);
    let perf = draw(conn, project, query, window, echoed)?;
    Ok(JsonConsolePlot {
        plot: plot.into_json_with(project, branches, testbeds, benchmarks, measures),
        perf,
    })
}

/// The dimensions of a page's plots, each plot's in its order, read once for the page.
struct Dimensions {
    branches: HashMap<PlotId, Vec<BranchUuid>>,
    testbeds: HashMap<PlotId, Vec<TestbedUuid>>,
    benchmarks: HashMap<PlotId, Vec<BenchmarkUuid>>,
    measures: HashMap<PlotId, Vec<MeasureUuid>>,
}

impl Dimensions {
    fn of(
        conn: &mut DbConnection,
        project: &QueryProject,
        plots: &[QueryPlot],
    ) -> Result<Self, HttpError> {
        if plots.is_empty() {
            return Ok(Self {
                branches: HashMap::new(),
                testbeds: HashMap::new(),
                benchmarks: HashMap::new(),
                measures: HashMap::new(),
            });
        }
        let ids = plots.iter().map(|plot| plot.id).collect::<Vec<_>>();
        Ok(Self {
            branches: grouped(
                schema::plot_branch::table
                    .inner_join(schema::branch::table)
                    .filter(schema::plot_branch::plot_id.eq_any(&ids))
                    .order((schema::plot_branch::plot_id, schema::plot_branch::rank))
                    .select((schema::plot_branch::plot_id, schema::branch::uuid))
                    .load(conn)
                    .map_err(resource_not_found_err!(PlotBranch, project))?,
            ),
            testbeds: grouped(
                schema::plot_testbed::table
                    .inner_join(schema::testbed::table)
                    .filter(schema::plot_testbed::plot_id.eq_any(&ids))
                    .order((schema::plot_testbed::plot_id, schema::plot_testbed::rank))
                    .select((schema::plot_testbed::plot_id, schema::testbed::uuid))
                    .load(conn)
                    .map_err(resource_not_found_err!(PlotTestbed, project))?,
            ),
            benchmarks: grouped(
                schema::plot_benchmark::table
                    .inner_join(schema::benchmark::table)
                    .filter(schema::plot_benchmark::plot_id.eq_any(&ids))
                    .order((
                        schema::plot_benchmark::plot_id,
                        schema::plot_benchmark::rank,
                    ))
                    .select((schema::plot_benchmark::plot_id, schema::benchmark::uuid))
                    .load(conn)
                    .map_err(resource_not_found_err!(PlotBenchmark, project))?,
            ),
            measures: grouped(
                schema::plot_measure::table
                    .inner_join(schema::measure::table)
                    .filter(schema::plot_measure::plot_id.eq_any(&ids))
                    .order((schema::plot_measure::plot_id, schema::plot_measure::rank))
                    .select((schema::plot_measure::plot_id, schema::measure::uuid))
                    .load(conn)
                    .map_err(resource_not_found_err!(PlotMeasure, project))?,
            ),
        })
    }
}

fn grouped<T>(rows: Vec<(PlotId, T)>) -> HashMap<PlotId, Vec<T>> {
    let mut grouped: HashMap<PlotId, Vec<T>> = HashMap::new();
    for (plot, uuid) in rows {
        grouped.entry(plot).or_default().push(uuid);
    }
    grouped
}
