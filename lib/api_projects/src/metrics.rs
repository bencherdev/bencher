use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    DateTime, JsonBoundary, JsonMetricTriple, JsonOneMetric, MetricName, MetricUuid,
    ProjectResourceId, ReportUuid,
    project::{alert::JsonPerfAlert, report::Iteration, threshold::JsonThresholdModel},
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
            parameter::QueryParameter,
            testbed::QueryTestbed,
            threshold::{
                QueryThreshold, alert::QueryAlert, boundary::QueryBoundary, model::QueryModel,
            },
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _, query_builder::QueryFragment,
    query_dsl::methods::LoadQuery, sqlite::Sqlite,
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
/// estimate, and the UUID of a bound or of any other named scalar is a row the view
/// does not have. Every name is addressable here.
///
/// Keep these joins flat with explicit `ON` clauses instead of nesting the threshold,
/// the model, and the alert inside the boundary join. `SQLite` cannot flatten a
/// compound right operand of an outer join, so the nested form makes it scan the
/// whole boundary table once per request, no matter how narrow the query is.
fn metric_query(
    project_id: ProjectId,
    metric_uuid: MetricUuid,
) -> impl LoadQuery<'static, DbConnection, MetricQuery> + QueryFragment<Sqlite> {
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
            schema::parameter::table
                .on(schema::parameter::id.eq(schema::report_benchmark::parameter_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(
            schema::head_version::table.on(schema::head_version::version_id.eq(schema::version::id)),
        )
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::head_version::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .inner_join(schema::measure::table.on(schema::measure::id.eq(schema::metric::measure_id)))
        // There may or may not be a boundary for the addressed row.
        .left_join(schema::boundary::table.on(schema::boundary::metric_id.eq(schema::metric::id)))
        .left_join(
            schema::threshold::table.on(schema::threshold::id.eq(schema::boundary::threshold_id)),
        )
        .left_join(schema::model::table.on(schema::model::id.eq(schema::boundary::model_id)))
        // There may or may not be an alert for any given boundary.
        .left_join(schema::alert::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
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
            QueryParameter::as_select(),
            QueryMeasure::as_select(),
            schema::report::uuid,
            schema::report_benchmark::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::spec_id,
            QueryMetric::as_select(),
            (
                (
                    schema::threshold::id,
                    schema::threshold::uuid,
                    schema::threshold::project_id,
                    schema::threshold::measure_id,
                    schema::threshold::branch_id,
                    schema::threshold::testbed_id,
                    schema::threshold::model_id,
                    schema::threshold::created,
                    schema::threshold::modified,
                ),
                (
                    schema::model::id,
                    schema::model::uuid,
                    schema::model::threshold_id,
                    schema::model::test,
                    schema::model::min_sample_size,
                    schema::model::max_sample_size,
                    schema::model::window,
                    schema::model::lower_boundary,
                    schema::model::upper_boundary,
                    schema::model::created,
                    schema::model::replaced,
                ),
                (
                    schema::boundary::id,
                    schema::boundary::uuid,
                    schema::boundary::metric_id,
                    schema::boundary::threshold_id,
                    schema::boundary::model_id,
                    schema::boundary::baseline,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                ),
                (
                    schema::alert::id,
                    schema::alert::uuid,
                    schema::alert::boundary_id,
                    schema::alert::boundary_limit,
                    schema::alert::status,
                    schema::alert::modified,
                )
                    .nullable(),
            )
                .nullable(),
        ))
        // The UUID is unique, so at most one row can match. The limit is what
        // `first` renders, kept here because the query is built apart from its run.
        .limit(1)
}

/// The gate on the addressed row: the threshold that gated it, the model that
/// threshold ran, the boundary it produced, and any alert that boundary raised.
type MetricGate = (
    QueryThreshold,
    QueryModel,
    QueryBoundary,
    Option<QueryAlert>,
);

type MetricQuery = (
    QueryBranch,
    QueryHead,
    QueryVersion,
    QueryTestbed,
    QueryBenchmark,
    QueryParameter,
    QueryMeasure,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    Option<SpecId>,
    QueryMetric,
    Option<MetricGate>,
);

fn metric_gate(
    project: &QueryProject,
    gate: Option<MetricGate>,
) -> (
    Option<JsonThresholdModel>,
    Option<JsonBoundary>,
    Option<JsonPerfAlert>,
) {
    if let Some((query_threshold, query_model, query_boundary, query_alert)) = gate {
        let threshold =
            Some(query_threshold.into_threshold_model_json_for_project(project, query_model));
        let boundary = Some(query_boundary.into_json());
        let alert = query_alert.map(QueryAlert::into_perf_json);
        (threshold, boundary, alert)
    } else {
        (None, None, None)
    }
}

/// The metric triple, for a `value` row and for nothing else.
///
/// The triple is a convention over three names, so it only means anything when the
/// address names the point estimate it is built around. Addressing a bound or any
/// other named scalar returns it absent: reconstructing the triple around a row the
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
        parameter,
        measure,
        report,
        iteration,
        start_time,
        end_time,
        spec_id,
        query_metric,
        gate,
    ): MetricQuery,
) -> Result<JsonOneMetric, HttpError> {
    let branch = branch.into_json_for_head(conn, project, &head, Some(version))?;
    let testbed = testbed.into_json_for_spec(conn, project, spec_id)?;
    let parameter = parameter.into_json_for_benchmark(&benchmark);
    let benchmark = benchmark.into_json_for_project(project);
    let measure = measure.into_json_for_project(project);

    let (threshold, boundary, alert) = metric_gate(project, gate);
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
        parameter,
        measure,
        name,
        value: value.into(),
        metric,
        threshold,
        boundary,
        alert,
    })
}

#[cfg(test)]
mod tests {
    use bencher_json::MetricUuid;
    use bencher_schema::model::project::ProjectId;
    use diesel::sqlite::Sqlite;

    use super::metric_query;

    fn metric_query_sql() -> String {
        diesel::debug_query::<Sqlite, _>(&metric_query(ProjectId::default(), MetricUuid::new()))
            .to_string()
    }

    /// The lookup drives on the `metric` table, so every named scalar is addressable.
    /// The `metric_boundary` view drives on `WHERE metric.name = 'value'`, which is
    /// exactly the rows a bound or any other name is not.
    #[test]
    fn metric_query_drives_on_the_metric_table() {
        let sql = metric_query_sql();
        assert!(
            sql.contains(
                "`metric` INNER JOIN `report_benchmark` ON (`report_benchmark`.`id` = `metric`.`report_benchmark_id`)"
            ),
            "{sql}"
        );
        assert!(!sql.contains("metric_boundary"), "{sql}");
        assert!(sql.contains("`metric`.`uuid` = ?"), "{sql}");
    }

    /// The boundary, its threshold, its model, and its alert hang off the addressed
    /// row as a flat chain of left joins. Nesting them renders the group in
    /// parentheses, and `SQLite` cannot flatten a compound right operand of an outer
    /// join: it materializes the whole subjoin, scanning the entire boundary table
    /// once per request. Pin the rendered shape so the nesting cannot come back
    /// unnoticed.
    #[test]
    fn metric_query_joins_the_boundary_flat() {
        let sql = metric_query_sql();
        assert!(
            sql.contains("LEFT OUTER JOIN `boundary` ON (`boundary`.`metric_id` = `metric`.`id`)"),
            "{sql}"
        );
        assert!(
            sql.contains(
                "LEFT OUTER JOIN `threshold` ON (`threshold`.`id` = `boundary`.`threshold_id`)"
            ),
            "{sql}"
        );
        assert!(
            sql.contains("LEFT OUTER JOIN `model` ON (`model`.`id` = `boundary`.`model_id`)"),
            "{sql}"
        );
        assert!(
            sql.contains("LEFT OUTER JOIN `alert` ON (`alert`.`boundary_id` = `boundary`.`id`)"),
            "{sql}"
        );
        assert!(!sql.contains("LEFT OUTER JOIN (("), "{sql}");
    }

    /// The parameter set the addressed row was measured under comes from the row's
    /// own report benchmark, never from a second query.
    #[test]
    fn metric_query_joins_the_parameter_set() {
        let sql = metric_query_sql();
        assert!(
            sql.contains(
                "INNER JOIN `parameter` ON (`parameter`.`id` = `report_benchmark`.`parameter_id`)"
            ),
            "{sql}"
        );
    }
}
