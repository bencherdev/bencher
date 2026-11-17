use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    DateTime, JsonMetricTriple, JsonOneMetric, MetricName, MetricUuid, ProjectResourceId,
    ReportUuid, project::report::Iteration,
};
use bencher_schema::model::spec::SpecId;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId, QueryProject,
            benchmark::QueryBenchmark,
            branch::{QueryBranch, head::QueryHead, version::QueryVersion},
            measure::QueryMeasure,
            metric::QueryMetric,
            testbed::QueryTestbed,
            variant::QueryVariant,
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
    query_dsl::methods::LoadQuery,
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a metric.
    pub metric: MetricUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metrics/{metric}",
    tags = ["projects", "metrics"]
}]
pub async fn proj_metric_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMetricParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a metric
///
/// View a metric for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metrics/{metric}",
    tags = ["projects", "metrics"]
}]
pub async fn proj_metric_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjMetricParams>,
) -> Result<ResponseOk<JsonOneMetric>, HttpError> {
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
    path_params: ProjMetricParams,
    api_actor: &ApiActor,
) -> Result<JsonOneMetric, HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    actor_conn!(context, api_actor, |conn| {
        metric_query(query_project.id, path_params.metric)
            .get_result::<MetricQuery>(conn)
            .map_err(resource_not_found_err!(
                Metric,
                (&query_project, &path_params.metric)
            ))
            .map(|metric_query| metric_query_json(conn, &query_project, metric_query))?
    })
}

/// Resolve one `metric` row by UUID, whatever it is named.
///
/// This drives on the `metric` table and not on the `metric_boundary` view: the view
/// keeps `WHERE metric.name = 'value'`, so it can only ever answer for a point
/// estimate, and the UUID of a bound or of any other metric is a row the view does
/// not have. Every name is addressable here.
fn metric_query(
    project_id: ProjectId,
    metric_uuid: MetricUuid,
) -> impl LoadQuery<'static, DbConnection, MetricQuery> {
    schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::benchmark::table
                .on(schema::benchmark::id.eq(schema::report_benchmark::benchmark_id)),
        )
        .inner_join(
            schema::variant::table
                .on(schema::variant::id.eq(schema::report_benchmark::variant_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        // The report's own head, not any head that shares its version: a branch that
        // served as a start point lends its versions to the new head as well.
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .inner_join(schema::measure::table.on(schema::measure::id.eq(schema::metric::measure_id)))
        .filter(schema::metric::uuid.eq(metric_uuid))
        // Make sure that the project is the same for all dimensions
        .filter(schema::branch::project_id.eq(project_id))
        .filter(schema::testbed::project_id.eq(project_id))
        .filter(schema::benchmark::project_id.eq(project_id))
        .filter(schema::measure::project_id.eq(project_id))
        .select((
            QueryBranch::as_select(),
            QueryHead::as_select(),
            QueryVersion::as_select(),
            QueryTestbed::as_select(),
            QueryBenchmark::as_select(),
            QueryVariant::as_select(),
            QueryMeasure::as_select(),
            schema::report::uuid,
            schema::report_benchmark::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::spec_id,
            QueryMetric::as_select(),
        ))
}

type MetricQuery = (
    QueryBranch,
    QueryHead,
    QueryVersion,
    QueryTestbed,
    QueryBenchmark,
    QueryVariant,
    QueryMeasure,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    Option<SpecId>,
    QueryMetric,
);

/// The metric triple, for a `value` row and for nothing else.
///
/// The triple is a convention over three names, so it only means anything when the
/// address names the point estimate it is built around. Addressing a bound or any
/// other metric returns it absent: reconstructing the triple around a row the
/// address does not name would assert numbers the caller never asked for.
fn metric_triple(
    conn: &mut DbConnection,
    query_metric: &QueryMetric,
) -> Result<Option<JsonMetricTriple>, HttpError> {
    if query_metric.name != MetricName::value() {
        return Ok(None);
    }

    query_metric.triple(conn).map(Some)
}

fn metric_query_json(
    conn: &mut DbConnection,
    project: &QueryProject,
    (
        branch,
        head,
        version,
        testbed,
        benchmark,
        variant,
        measure,
        report,
        iteration,
        start_time,
        end_time,
        spec_id,
        query_metric,
    ): MetricQuery,
) -> Result<JsonOneMetric, HttpError> {
    let branch = branch.into_json_for_head(conn, project, &head, Some(version))?;
    let testbed = testbed.into_json_for_spec(conn, project, spec_id)?;
    let variant = variant.into_json_for_benchmark(&benchmark);
    let benchmark = benchmark.into_json_for_project(project);
    let measure = measure.into_json_for_project(project);

    let metric = metric_triple(conn, &query_metric)?;
    let QueryMetric {
        id: _,
        uuid,
        report_benchmark_id: _,
        measure_id: _,
        name,
        value,
    } = query_metric;

    Ok(JsonOneMetric {
        uuid,
        report,
        iteration,
        start_time,
        end_time,
        branch,
        testbed,
        benchmark,
        variant,
        measure,
        name,
        value: value.into(),
        metric,
    })
}
