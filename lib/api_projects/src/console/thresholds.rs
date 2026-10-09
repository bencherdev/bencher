use std::collections::HashMap;

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BranchName, BranchSlug, BranchUuid, DateTime, DateTimeMillis, MeasureSlug, MeasureUuid, Model,
    ProjectResourceId, ResourceName, TestbedSlug, TestbedUuid, ThresholdUuid,
    project::{
        alert::AlertStatus,
        threshold::{
            DEFAULT_CONSOLE_THRESHOLDS_PER_PAGE, JsonConsoleThreshold, JsonConsoleThresholdBranch,
            JsonConsoleThresholdMeasure, JsonConsoleThresholdModel, JsonConsoleThresholdRow,
            JsonConsoleThresholdRowModel, JsonConsoleThresholdTestbed, JsonConsoleThresholds,
            JsonConsoleThresholdsQueryParams,
        },
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId, QueryProject,
            branch::BranchId,
            measure::MeasureId,
            testbed::TestbedId,
            threshold::{QueryThreshold, ThresholdId, model::QueryModel},
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    BoolExpressionMethods as _, BoxableExpression, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableExpression,
    SelectableHelper as _, dsl::count_star, sql_types::Bool, sqlite::Sqlite,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::alerts::archived_filter;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleThresholdsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/thresholds",
    tags = ["projects", "thresholds"],
    unpublished = true,
}]
pub async fn proj_console_thresholds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleThresholdsParams>,
    _query_params: Query<JsonConsoleThresholdsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List thresholds for the console
///
/// List a page of the thresholds of a project, oldest first, with the alerts each raised
/// inside a window.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/thresholds",
    tags = ["projects", "thresholds"],
    unpublished = true,
}]
pub async fn proj_console_thresholds_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleThresholdsParams>,
    query_params: Query<JsonConsoleThresholdsQueryParams>,
) -> Result<ResponseOk<JsonConsoleThresholds>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_ls_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_ls_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleThresholdsParams,
    query_params: JsonConsoleThresholdsQueryParams,
) -> Result<JsonConsoleThresholds, HttpError> {
    let window = AlertWindow::new(query_params.start_time, query_params.end_time)?;
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;

        let (rows, total) = page(conn, &query_project, &query_params)?;

        let threshold_ids = rows
            .iter()
            .map(|(threshold, ..)| threshold.id)
            .collect::<Vec<_>>();
        let raised = raised_counts(conn, &threshold_ids, window)?;
        let active = active_counts(conn, &threshold_ids)?;
        let branch_ids = rows
            .iter()
            .map(|(_, branch, ..)| branch.id)
            .collect::<Vec<_>>();
        let start_points = start_points(conn, &branch_ids)?;

        let mut tables = Tables::default();
        let thresholds = rows
            .into_iter()
            .map(|(threshold, branch, testbed, measure, model)| {
                let count = |counts: &HashMap<ThresholdId, i64>| {
                    counts
                        .get(&threshold.id)
                        .map_or(0, |&count| u32::try_from(count).unwrap_or(u32::MAX))
                };
                JsonConsoleThresholdRow {
                    uuid: threshold.uuid,
                    branch: tables.branch(branch, &start_points),
                    testbed: tables.testbed(testbed),
                    measure: tables.measure(measure),
                    parameters: threshold.parameters,
                    metric: threshold.metric,
                    model: model.map(row_model_json),
                    raised: count(&raised),
                    active: count(&active),
                }
            })
            .collect();

        Ok(JsonConsoleThresholds {
            total: u32::try_from(total).unwrap_or(u32::MAX),
            thresholds,
            branches: tables.branches,
            testbeds: tables.testbeds,
            measures: tables.measures,
        })
    })
}

fn page(
    conn: &mut DbConnection,
    query_project: &QueryProject,
    query_params: &JsonConsoleThresholdsQueryParams,
) -> Result<(Vec<PageRow>, i64), HttpError> {
    let per_page = query_params
        .per_page
        .unwrap_or(DEFAULT_CONSOLE_THRESHOLDS_PER_PAGE);
    let offset = i64::from(query_params.page.unwrap_or(1).saturating_sub(1)) * i64::from(per_page);
    let rows = schema::threshold::table
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::threshold::branch_id)))
        .inner_join(
            schema::testbed::table.on(schema::testbed::id.eq(schema::threshold::testbed_id)),
        )
        .inner_join(
            schema::measure::table.on(schema::measure::id.eq(schema::threshold::measure_id)),
        )
        .left_join(
            schema::model::table.on(schema::model::id.nullable().eq(schema::threshold::model_id)),
        )
        .filter(matching(query_project.id, query_params))
        .order((
            schema::threshold::created.asc(),
            schema::threshold::id.asc(),
        ))
        .offset(offset)
        .limit(i64::from(per_page))
        .select((
            QueryThreshold::as_select(),
            BranchRow::as_select(),
            TestbedRow::as_select(),
            MeasureRow::as_select(),
            Option::<QueryModel>::as_select(),
        ))
        .load::<PageRow>(conn)
        .map_err(resource_not_found_err!(
            Threshold,
            (query_project, query_params)
        ))?;
    let total = schema::threshold::table
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::threshold::branch_id)))
        .inner_join(
            schema::testbed::table.on(schema::testbed::id.eq(schema::threshold::testbed_id)),
        )
        .inner_join(
            schema::measure::table.on(schema::measure::id.eq(schema::threshold::measure_id)),
        )
        .filter(matching(query_project.id, query_params))
        .select(count_star())
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(
            Threshold,
            (query_project, query_params)
        ))?;
    Ok((rows, total))
}

type PageRow = (
    QueryThreshold,
    BranchRow,
    TestbedRow,
    MeasureRow,
    Option<QueryModel>,
);

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleThresholdParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a threshold.
    pub threshold: ThresholdUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/thresholds/{threshold}",
    tags = ["projects", "thresholds"],
    unpublished = true,
}]
pub async fn proj_console_threshold_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleThresholdParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a threshold for the console
///
/// View a threshold of a project with every model it has had, newest first.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/thresholds/{threshold}",
    tags = ["projects", "thresholds"],
    unpublished = true,
}]
pub async fn proj_console_threshold_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleThresholdParams>,
) -> Result<ResponseOk<JsonConsoleThreshold>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), &api_actor, path_params.into_inner())
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleThresholdParams,
) -> Result<JsonConsoleThreshold, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = readable_project(conn, context, &path_params.project, api_actor)?;

        let (threshold, branch, testbed, measure) = schema::threshold::table
            .inner_join(
                schema::branch::table.on(schema::branch::id.eq(schema::threshold::branch_id)),
            )
            .inner_join(
                schema::testbed::table.on(schema::testbed::id.eq(schema::threshold::testbed_id)),
            )
            .inner_join(
                schema::measure::table.on(schema::measure::id.eq(schema::threshold::measure_id)),
            )
            .filter(schema::threshold::project_id.eq(query_project.id))
            .filter(schema::threshold::uuid.eq(path_params.threshold))
            .select((
                QueryThreshold::as_select(),
                BranchRow::as_select(),
                TestbedRow::as_select(),
                MeasureRow::as_select(),
            ))
            .first::<(QueryThreshold, BranchRow, TestbedRow, MeasureRow)>(conn)
            .map_err(resource_not_found_err!(
                Threshold,
                (&query_project, path_params.threshold)
            ))?;
        let models = schema::model::table
            .filter(schema::model::threshold_id.eq(threshold.id))
            .order((schema::model::created.desc(), schema::model::id.desc()))
            .select(QueryModel::as_select())
            .load::<QueryModel>(conn)
            .map_err(resource_not_found_err!(Model, threshold.id))?;
        let start_points = start_points(conn, &[branch.id])?;

        let model = models
            .iter()
            .find(|model| Some(model.id) == threshold.model_id)
            .copied()
            .map(model_json);
        Ok(JsonConsoleThreshold {
            uuid: threshold.uuid,
            branch: branch.into_json(&start_points),
            testbed: testbed.into_json(),
            measure: measure.into_json(),
            parameters: threshold.parameters,
            metric: threshold.metric,
            model,
            models: models.into_iter().map(model_json).collect(),
            created: threshold.created.into(),
            modified: threshold.modified.into(),
        })
    })
}

/// The project when the actor may read it, which takes a login even on a public project.
pub(super) fn readable_project(
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

fn matching<QS: 'static>(
    project_id: ProjectId,
    query_params: &JsonConsoleThresholdsQueryParams,
) -> Box<dyn BoxableExpression<QS, Sqlite, SqlType = Bool>>
where
    schema::threshold::project_id: SelectableExpression<QS>,
    schema::branch::uuid: SelectableExpression<QS>,
    schema::branch::archived: SelectableExpression<QS>,
    schema::testbed::uuid: SelectableExpression<QS>,
    schema::testbed::archived: SelectableExpression<QS>,
    schema::measure::uuid: SelectableExpression<QS>,
    schema::measure::archived: SelectableExpression<QS>,
{
    let mut matching: Box<dyn BoxableExpression<QS, Sqlite, SqlType = Bool>> = Box::new(
        schema::threshold::project_id
            .eq(project_id)
            .and(archived_filter(query_params.archived)),
    );
    if let Some(branch) = query_params.branch {
        matching = Box::new(matching.and(schema::branch::uuid.eq(branch)));
    }
    if let Some(testbed) = query_params.testbed {
        matching = Box::new(matching.and(schema::testbed::uuid.eq(testbed)));
    }
    if let Some(measure) = query_params.measure {
        matching = Box::new(matching.and(schema::measure::uuid.eq(measure)));
    }
    matching
}

/// When the reports that raised the counted alerts were created, inclusive at both ends.
#[derive(Clone, Copy)]
struct AlertWindow {
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
}

impl AlertWindow {
    fn new(
        start_time: Option<DateTimeMillis>,
        end_time: Option<DateTimeMillis>,
    ) -> Result<Self, HttpError> {
        if let (Some(start_time), Some(end_time)) = (start_time, end_time)
            && i64::from(start_time) > i64::from(end_time)
        {
            return Err(bad_request_error(format!(
                "The window starts ({start_time}) after it ends ({end_time})"
            )));
        }
        Ok(Self {
            start_time: start_time.map(DateTime::from),
            end_time: end_time.map(DateTime::from),
        })
    }
}

/// The alerts each threshold raised inside the window, whatever their status now.
fn raised_counts(
    conn: &mut DbConnection,
    threshold_ids: &[ThresholdId],
    window: AlertWindow,
) -> Result<HashMap<ThresholdId, i64>, HttpError> {
    if threshold_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut query = schema::alert::table
        .inner_join(schema::boundary::table.on(schema::boundary::id.eq(schema::alert::boundary_id)))
        .inner_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .filter(schema::alert::threshold_id.eq_any(threshold_ids))
        .group_by(schema::alert::threshold_id)
        .select((schema::alert::threshold_id, count_star()))
        .into_boxed();
    if let Some(start_time) = window.start_time {
        query = query.filter(schema::report::created.ge(start_time));
    }
    if let Some(end_time) = window.end_time {
        query = query.filter(schema::report::created.le(end_time));
    }
    query
        .load::<(ThresholdId, i64)>(conn)
        .map(|counts| counts.into_iter().collect())
        .map_err(resource_not_found_err!(Alert, threshold_ids))
}

/// The alerts of each threshold that are active now, whenever they were raised.
fn active_counts(
    conn: &mut DbConnection,
    threshold_ids: &[ThresholdId],
) -> Result<HashMap<ThresholdId, i64>, HttpError> {
    if threshold_ids.is_empty() {
        return Ok(HashMap::new());
    }
    schema::alert::table
        .filter(schema::alert::threshold_id.eq_any(threshold_ids))
        .filter(schema::alert::status.eq(AlertStatus::Active))
        .group_by(schema::alert::threshold_id)
        .select((schema::alert::threshold_id, count_star()))
        .load::<(ThresholdId, i64)>(conn)
        .map(|counts| counts.into_iter().collect())
        .map_err(resource_not_found_err!(Alert, threshold_ids))
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

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::branch)]
struct BranchRow {
    id: BranchId,
    uuid: BranchUuid,
    name: BranchName,
    slug: BranchSlug,
    archived: Option<DateTime>,
}

impl BranchRow {
    fn into_json(self, start_points: &HashMap<BranchId, BranchName>) -> JsonConsoleThresholdBranch {
        JsonConsoleThresholdBranch {
            start_point: start_points.get(&self.id).cloned(),
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            archived: self.archived.map(Into::into),
        }
    }
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::testbed)]
struct TestbedRow {
    id: TestbedId,
    uuid: TestbedUuid,
    name: ResourceName,
    slug: TestbedSlug,
    archived: Option<DateTime>,
}

impl TestbedRow {
    fn into_json(self) -> JsonConsoleThresholdTestbed {
        JsonConsoleThresholdTestbed {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            archived: self.archived.map(Into::into),
        }
    }
}

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = schema::measure)]
struct MeasureRow {
    id: MeasureId,
    uuid: MeasureUuid,
    name: ResourceName,
    slug: MeasureSlug,
    units: ResourceName,
    archived: Option<DateTime>,
}

impl MeasureRow {
    fn into_json(self) -> JsonConsoleThresholdMeasure {
        JsonConsoleThresholdMeasure {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            units: self.units,
            archived: self.archived.map(Into::into),
        }
    }
}

/// The branches, testbeds, and measures of a page in the order its rows first refer to them.
#[derive(Default)]
struct Tables {
    branch_index: HashMap<BranchId, u32>,
    branches: Vec<JsonConsoleThresholdBranch>,
    testbed_index: HashMap<TestbedId, u32>,
    testbeds: Vec<JsonConsoleThresholdTestbed>,
    measure_index: HashMap<MeasureId, u32>,
    measures: Vec<JsonConsoleThresholdMeasure>,
}

impl Tables {
    fn branch(&mut self, branch: BranchRow, start_points: &HashMap<BranchId, BranchName>) -> u32 {
        *self.branch_index.entry(branch.id).or_insert_with(|| {
            self.branches.push(branch.into_json(start_points));
            index(self.branches.len())
        })
    }

    fn testbed(&mut self, testbed: TestbedRow) -> u32 {
        *self.testbed_index.entry(testbed.id).or_insert_with(|| {
            self.testbeds.push(testbed.into_json());
            index(self.testbeds.len())
        })
    }

    fn measure(&mut self, measure: MeasureRow) -> u32 {
        *self.measure_index.entry(measure.id).or_insert_with(|| {
            self.measures.push(measure.into_json());
            index(self.measures.len())
        })
    }
}

fn index(len: usize) -> u32 {
    u32::try_from(len.saturating_sub(1)).unwrap_or(u32::MAX)
}

fn row_model_json(model: QueryModel) -> JsonConsoleThresholdRowModel {
    let Model {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
    } = model.into_model();
    JsonConsoleThresholdRowModel {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
    }
}

fn model_json(model: QueryModel) -> JsonConsoleThresholdModel {
    let QueryModel {
        uuid,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        created,
        replaced,
        ..
    } = model;
    JsonConsoleThresholdModel {
        uuid,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        created: created.into(),
        replaced: replaced.map(Into::into),
    }
}
