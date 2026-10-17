use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BranchName, BranchSlug, BranchUuid, DateTime, DateTimeMillis, GitHash, HeadUuid, NameId,
    ProjectResourceId, ReportUuid, ResourceName, TestbedNameId, TestbedSlug, TestbedUuid, UserName,
    VersionUuid,
    project::{
        alert::AlertStatus,
        console::{
            ConsoleAuthorKind, DEFAULT_CONSOLE_RUNS_PER_PAGE, JsonConsoleAlerts, JsonConsoleAuthor,
            JsonConsoleBranch, JsonConsoleRun, JsonConsoleRunLink, JsonConsoleRunQueryParams,
            JsonConsoleRunReport, JsonConsoleRunSummary, JsonConsoleRuns, JsonConsoleRunsQuery,
            JsonConsoleRunsQueryParams, JsonConsoleTestbed, MAX_CONSOLE_RUN_NAMES,
        },
        head::VersionNumber,
        report::{Adapter, Iteration, JsonReportAlertsCounts, ReportAlerts},
    },
};
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId,
            branch::version::VersionId,
            key::ProjectKeyId,
            report::ReportId,
            testbed::{QueryTestbed, TestbedId},
        },
        user::{
            UserId,
            actor::{ApiActor, PubProjectBearerToken},
        },
    },
    schema,
};
use diesel::{
    BoolExpressionMethods as _, BoxableExpression, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableExpression,
    dsl::{count, count_star, min},
    sql_types::Bool,
    sqlite::Sqlite,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{alerts::run_alerts, before, thresholds::readable_project};

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleRunsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/console/projects/{project}/runs",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_runs_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleRunsParams>,
    _query_params: Query<JsonConsoleRunsQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List runs for the console
///
/// A page of a project's runs, newest first by their first report's start. A run
/// is every report with one version. Reports on an archived branch or testbed
/// are left out, as the reports list leaves them out.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/console/projects/{project}/runs",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_runs_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleRunsParams>,
    query_params: Query<JsonConsoleRunsQueryParams>,
) -> Result<ResponseOk<JsonConsoleRuns>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;
    let json = get_ls_inner(rqctx.context(), &api_actor, path_params.into_inner(), query)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_ls_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleRunsParams,
    query: JsonConsoleRunsQuery,
) -> Result<JsonConsoleRuns, HttpError> {
    if let (Some(start_time), Some(end_time)) = (query.start_time, query.end_time)
        && i64::from(start_time) > i64::from(end_time)
    {
        return Err(bad_request_error(format!(
            "The window starts ({start_time}) after it ends ({end_time})"
        )));
    }
    let conn = actor_conn!(context, api_actor);
    let project = readable_project(conn, context, &path_params.project, api_actor)?;

    let mut keys = run_keys(conn, project.id, &query)
        .map_err(resource_not_found_err!(Report, (&project, &query)))?
        .into_iter()
        .filter_map(|(version_id, start_time)| {
            let start_time = start_time?;
            in_window(&query, start_time).then_some((version_id, start_time))
        })
        .collect::<Vec<_>>();
    keys.sort_unstable_by_key(|&(version_id, start_time)| {
        std::cmp::Reverse((start_time.timestamp(), version_id))
    });
    let total = to_u32(keys.len());
    let (_, page) = page_of(&keys, query.page, query.per_page);
    let page = page
        .iter()
        .map(|(version_id, _)| *version_id)
        .collect::<Vec<_>>();
    if page.is_empty() {
        return Ok(JsonConsoleRuns {
            total,
            runs: Vec::new(),
        });
    }

    let versions =
        versions(conn, &page).map_err(resource_not_found_err!(Version, (&project, &page)))?;
    let rows =
        report_rows(conn, &page).map_err(resource_not_found_err!(Report, (&project, &page)))?;
    let lines =
        report_lines(conn, &page).map_err(resource_not_found_err!(Metric, (&project, &page)))?;
    let alerts =
        report_alerts(conn, &page).map_err(resource_not_found_err!(Alert, (&project, &page)))?;

    let mut by_version = HashMap::<VersionId, Vec<ReportRow>>::new();
    for row in rows {
        by_version.entry(row.version_id).or_default().push(row);
    }
    let runs = page
        .iter()
        .filter_map(|version_id| {
            let version = versions.get(version_id)?;
            let rows = by_version.remove(version_id)?;
            summary(version, &rows, &lines, &alerts)
        })
        .collect();
    Ok(JsonConsoleRuns { total, runs })
}

fn in_window(query: &JsonConsoleRunsQuery, start_time: DateTime) -> bool {
    let start = start_time.timestamp();
    query
        .start_time
        .is_none_or(|bound| DateTime::from(bound).timestamp() <= start)
        && query
            .end_time
            .is_none_or(|bound| start <= DateTime::from(bound).timestamp())
}

/// One page of the items, and how many come before it.
fn page_of<T>(items: &[T], page: Option<u32>, per_page: Option<u8>) -> (usize, &[T]) {
    let per_page = usize::from(per_page.unwrap_or(DEFAULT_CONSOLE_RUNS_PER_PAGE));
    let page = usize::try_from(page.unwrap_or(1).max(1) - 1).unwrap_or(usize::MAX);
    let start = page.saturating_mul(per_page).min(items.len());
    let end = start.saturating_add(per_page).min(items.len());
    (start, items.get(start..end).unwrap_or_default())
}

/// Every matching run and its first report's start: the window is checked on
/// that start, so a run's earlier reports are never cut away by a filter.
fn run_keys(
    conn: &mut DbConnection,
    project_id: ProjectId,
    query: &JsonConsoleRunsQuery,
) -> diesel::QueryResult<Vec<(VersionId, Option<DateTime>)>> {
    // The filters ask which runs hold a matching report, of the same tables the
    // keys are read from.
    let (run_report, run_testbed) =
        diesel::alias!(schema::report as run_report, schema::testbed as run_testbed);
    let run_version = run_report.field(schema::report::version_id);
    let run_end = run_report.field(schema::report::end_time);
    // A run that started in the window ends in it or after it, so each of its
    // reports is found by end time, which the report indexes lead with.
    let ends_after = query.start_time.map(DateTime::from);

    let mut keys = schema::report::table
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .filter(schema::branch::archived.is_null())
        .filter(schema::testbed::archived.is_null())
        .group_by(schema::report::version_id)
        .select((schema::report::version_id, min(schema::report::start_time)))
        .into_boxed();
    if let Some(branch) = &query.branch {
        keys = match branch {
            NameId::Uuid(uuid) => keys.filter(schema::branch::uuid.eq(uuid.to_string())),
            NameId::Slug(slug) => keys.filter(schema::branch::slug.eq(slug.to_string())),
            NameId::Name(name) => keys.filter(schema::branch::name.eq(name.to_string())),
        };
    }
    if let Some(testbed) = &query.testbed {
        // A testbed that is not there, or archived, holds no runs to list.
        let Some(testbed_id) = visible_testbed(conn, project_id, testbed)? else {
            return Ok(Vec::new());
        };
        let mut on_testbed = run_report
            .filter(run_report.field(schema::report::testbed_id).eq(testbed_id))
            .select(run_version)
            .into_boxed();
        if let Some(start) = ends_after {
            on_testbed = on_testbed.filter(run_end.ge(start));
        }
        keys = keys.filter(schema::report::version_id.eq_any(on_testbed));
    }
    if let Some(adapter) = query.adapter {
        let mut from_adapter = run_report
            .inner_join(
                run_testbed.on(run_testbed
                    .field(schema::testbed::id)
                    .eq(run_report.field(schema::report::testbed_id))),
            )
            .filter(run_report.field(schema::report::project_id).eq(project_id))
            .filter(
                run_report
                    .field(schema::report::adapter)
                    .eq_any(adapter.listed_from()),
            )
            .filter(run_testbed.field(schema::testbed::archived).is_null())
            .select(run_version)
            .into_boxed();
        if let Some(start) = ends_after {
            from_adapter = from_adapter.filter(run_end.ge(start));
        }
        keys = keys.filter(schema::report::version_id.eq_any(from_adapter));
    }
    if let Some(ReportAlerts::Active) = query.alerts {
        keys = keys.filter(
            schema::report::version_id.eq_any(
                schema::alert::table
                    .inner_join(
                        run_report.on(run_report
                            .field(schema::report::id)
                            .eq(schema::alert::report_id)),
                    )
                    .inner_join(
                        run_testbed.on(run_testbed
                            .field(schema::testbed::id)
                            .eq(run_report.field(schema::report::testbed_id))),
                    )
                    .filter(schema::alert::project_id.eq(project_id))
                    .filter(schema::alert::status.eq(AlertStatus::Active))
                    .filter(run_testbed.field(schema::testbed::archived).is_null())
                    .select(run_version),
            ),
        );
    }
    if let Some(start) = ends_after {
        keys = keys.filter(
            schema::report::version_id.eq_any(
                run_report
                    .filter(run_report.field(schema::report::project_id).eq(project_id))
                    .filter(run_end.ge(start))
                    .select(run_version),
            ),
        );
    }
    // Each set above holds only the project's runs, so the runs are read by
    // version from the narrowest of them rather than from every report.
    if query.testbed.is_none()
        && query.adapter.is_none()
        && query.alerts.is_none()
        && ends_after.is_none()
    {
        keys = keys.filter(schema::report::project_id.eq(project_id));
    }
    keys.load(conn)
}

fn visible_testbed(
    conn: &mut DbConnection,
    project_id: ProjectId,
    testbed: &TestbedNameId,
) -> diesel::QueryResult<Option<TestbedId>> {
    schema::testbed::table
        .filter(schema::testbed::project_id.eq(project_id))
        .filter(schema::testbed::archived.is_null())
        .filter(QueryTestbed::eq_name_id(testbed))
        .select(schema::testbed::id)
        .first(conn)
        .optional()
}

/// The reports the reports list shows: none on an archived branch or testbed.
fn visible<QS>() -> Box<dyn BoxableExpression<QS, Sqlite, SqlType = Bool>>
where
    schema::branch::archived: SelectableExpression<QS>,
    schema::testbed::archived: SelectableExpression<QS>,
{
    Box::new(
        schema::branch::archived
            .is_null()
            .and(schema::testbed::archived.is_null()),
    )
}

type VersionRow = (VersionId, VersionUuid, VersionNumber, Option<GitHash>);

fn versions(
    conn: &mut DbConnection,
    version_ids: &[VersionId],
) -> diesel::QueryResult<HashMap<VersionId, VersionRow>> {
    Ok(schema::version::table
        .filter(schema::version::id.eq_any(version_ids))
        .select((
            schema::version::id,
            schema::version::uuid,
            schema::version::number,
            schema::version::hash,
        ))
        .load::<VersionRow>(conn)?
        .into_iter()
        .map(|row| (row.0, row))
        .collect())
}

/// A visible report of a run, with what its row and its run's row show.
pub(super) struct ReportRow {
    pub version_id: VersionId,
    pub id: ReportId,
    pub uuid: ReportUuid,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub created: DateTime,
    pub branch: JsonConsoleBranch,
    pub testbed: JsonConsoleTestbed,
    pub adapter: Adapter,
    pub author: Option<Author>,
}

/// Who sent a report, by identity so two people of one name stay two.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Author {
    User(UserId, String),
    Key(ProjectKeyId, String),
}

impl Author {
    fn json(&self) -> JsonConsoleAuthor {
        match self {
            Self::User(_, name) => JsonConsoleAuthor {
                kind: ConsoleAuthorKind::User,
                name: name.clone(),
            },
            Self::Key(_, name) => JsonConsoleAuthor {
                kind: ConsoleAuthorKind::Key,
                name: name.clone(),
            },
        }
    }
}

type ReportRowColumns = (
    (
        VersionId,
        ReportId,
        ReportUuid,
        DateTime,
        DateTime,
        DateTime,
    ),
    (BranchUuid, BranchName, BranchSlug, HeadUuid),
    (TestbedUuid, ResourceName, TestbedSlug),
    Adapter,
    (Option<UserId>, Option<UserName>),
    (Option<ProjectKeyId>, Option<ResourceName>),
);

/// The visible reports of the runs, each run's in start order.
pub(super) fn report_rows(
    conn: &mut DbConnection,
    version_ids: &[VersionId],
) -> diesel::QueryResult<Vec<ReportRow>> {
    let rows = schema::report::table
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .left_join(schema::user::table.on(schema::report::user_id.eq(schema::user::id.nullable())))
        .left_join(
            schema::project_key::table
                .on(schema::report::project_key_id.eq(schema::project_key::id.nullable())),
        )
        .filter(schema::report::version_id.eq_any(version_ids))
        .filter(visible())
        .order((
            schema::report::version_id,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::created,
            schema::report::id,
        ))
        .select((
            (
                schema::report::version_id,
                schema::report::id,
                schema::report::uuid,
                schema::report::start_time,
                schema::report::end_time,
                schema::report::created,
            ),
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
            schema::report::adapter,
            (schema::user::id.nullable(), schema::user::name.nullable()),
            (
                schema::project_key::id.nullable(),
                schema::project_key::name.nullable(),
            ),
        ))
        .load::<ReportRowColumns>(conn)?;
    Ok(rows
        .into_iter()
        .map(
            |(
                (version_id, id, uuid, start_time, end_time, created),
                (branch_uuid, branch_name, branch_slug, head),
                (testbed_uuid, testbed_name, testbed_slug),
                adapter,
                user,
                key,
            )| {
                let author = match (key, user) {
                    ((Some(key_id), Some(name)), _) => Some(Author::Key(key_id, name.to_string())),
                    (_, (Some(user_id), Some(name))) => {
                        Some(Author::User(user_id, name.to_string()))
                    },
                    _ => None,
                };
                ReportRow {
                    version_id,
                    id,
                    uuid,
                    start_time,
                    end_time,
                    created,
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
                    adapter: adapter.normalize(),
                    author,
                }
            },
        )
        .collect())
}

/// The runs' visible reports, as a subquery so a run of many reports binds one value.
macro_rules! visible_report_ids {
    ($version_ids:expr) => {
        schema::report::table
            .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
            .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
            .inner_join(
                schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)),
            )
            .filter(schema::report::version_id.eq_any($version_ids))
            .filter(schema::branch::archived.is_null())
            .filter(schema::testbed::archived.is_null())
            .select(schema::report::id)
    };
}

/// Each report's lines as the reports list counts them: every iteration reports
/// the same lines, so the most any iteration reported.
fn report_lines(
    conn: &mut DbConnection,
    version_ids: &[VersionId],
) -> diesel::QueryResult<HashMap<ReportId, u32>> {
    let counts = schema::report_benchmark::table
        .inner_join(schema::metric::table)
        .filter(schema::report_benchmark::report_id.eq_any(visible_report_ids!(version_ids)))
        .group_by((
            schema::report_benchmark::report_id,
            schema::report_benchmark::iteration,
        ))
        .select((
            schema::report_benchmark::report_id,
            schema::report_benchmark::iteration,
            count(schema::metric::id),
        ))
        .load::<(ReportId, Iteration, i64)>(conn)?;
    let mut lines = HashMap::<ReportId, u32>::new();
    for (report_id, _iteration, count) in counts {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let most = lines.entry(report_id).or_default();
        *most = (*most).max(count);
    }
    Ok(lines)
}

/// Each report's alerts, by status.
fn report_alerts(
    conn: &mut DbConnection,
    version_ids: &[VersionId],
) -> diesel::QueryResult<HashMap<ReportId, JsonReportAlertsCounts>> {
    let counts = schema::alert::table
        .filter(schema::alert::report_id.eq_any(visible_report_ids!(version_ids)))
        .group_by((schema::alert::report_id, schema::alert::status))
        .select((
            schema::alert::report_id,
            schema::alert::status,
            count_star(),
        ))
        .load::<(ReportId, AlertStatus, i64)>(conn)?;
    let mut alerts = HashMap::<ReportId, JsonReportAlertsCounts>::new();
    for (report_id, status, count) in counts {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let counts = alerts.entry(report_id).or_default();
        counts.total = counts.total.saturating_add(count);
        if matches!(status, AlertStatus::Active) {
            counts.active = counts.active.saturating_add(count);
        }
    }
    Ok(alerts)
}

fn add_alerts(sum: &mut JsonReportAlertsCounts, counts: Option<&JsonReportAlertsCounts>) {
    if let Some(counts) = counts {
        sum.active = sum.active.saturating_add(counts.active);
        sum.total = sum.total.saturating_add(counts.total);
    }
}

/// Distinct values in the order first seen: at most the cap of them, and how many there are.
fn first_seen<K: Clone + Eq + std::hash::Hash, I: Iterator<Item = K>>(items: I) -> (Vec<K>, u32) {
    let mut seen = HashSet::new();
    let mut named = Vec::new();
    for item in items {
        if seen.insert(item.clone()) && named.len() < MAX_CONSOLE_RUN_NAMES {
            named.push(item);
        }
    }
    (named, to_u32(seen.len()))
}

/// A run's row from its visible reports in start order.
fn summary(
    version: &VersionRow,
    rows: &[ReportRow],
    lines: &HashMap<ReportId, u32>,
    alerts: &HashMap<ReportId, JsonReportAlertsCounts>,
) -> Option<JsonConsoleRunSummary> {
    let (_, uuid, number, hash) = version.clone();
    let facts = RunFacts::new(rows, lines, alerts)?;
    let (testbeds, testbed_count) = first_seen(
        rows.iter()
            .map(|row| (row.testbed.uuid, row.testbed.name.clone())),
    );
    let (adapters, adapter_count) = first_seen(rows.iter().map(|row| row.adapter));
    let (authors, author_count) = first_seen(rows.iter().filter_map(|row| row.author.clone()));
    Some(JsonConsoleRunSummary {
        uuid,
        number,
        hash,
        branch: facts.branch,
        start_time: facts.start_time,
        end_time: facts.end_time,
        reports: to_u32(rows.len()),
        testbeds: testbeds.into_iter().map(|(_, name)| name).collect(),
        testbed_count,
        adapters,
        adapter_count,
        authors: authors.iter().map(Author::json).collect(),
        author_count,
        lines: facts.lines,
        alerts: facts.alerts,
    })
}

/// What a run's heading and its row both show.
struct RunFacts {
    branch: JsonConsoleBranch,
    start_time: DateTimeMillis,
    end_time: DateTimeMillis,
    lines: u32,
    alerts: JsonReportAlertsCounts,
}

impl RunFacts {
    /// From a run's visible reports in start order; a run with none has no facts.
    fn new(
        rows: &[ReportRow],
        lines: &HashMap<ReportId, u32>,
        alerts: &HashMap<ReportId, JsonReportAlertsCounts>,
    ) -> Option<Self> {
        // The first report's branch, so a version that predates per branch versions
        // still names one branch.
        let first = rows.first()?;
        let end_time = rows
            .iter()
            .map(|row| row.end_time)
            .max_by_key(DateTime::timestamp)?;
        let mut sum = JsonReportAlertsCounts::default();
        let mut total_lines = 0u32;
        for row in rows {
            total_lines =
                total_lines.saturating_add(lines.get(&row.id).copied().unwrap_or_default());
            add_alerts(&mut sum, alerts.get(&row.id));
        }
        Some(Self {
            branch: first.branch.clone(),
            start_time: first.start_time.into(),
            end_time: end_time.into(),
            lines: total_lines,
            alerts: sum,
        })
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleRunParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for the run's version.
    pub version: VersionUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/console/projects/{project}/runs/{version}",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_run_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleRunParams>,
    _query_params: Query<JsonConsoleRunQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a run for the console
///
/// A run's facts and one page of its reports in start order, and on the first
/// page the runs before and after it on its branch and the alerts its reports
/// raised. Reports on an archived branch or testbed are left out, as the runs
/// list leaves them out.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/console/projects/{project}/runs/{version}",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_run_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleRunParams>,
    query_params: Query<JsonConsoleRunQueryParams>,
) -> Result<ResponseOk<JsonConsoleRun>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleRunParams,
    query_params: JsonConsoleRunQueryParams,
) -> Result<JsonConsoleRun, HttpError> {
    let conn = actor_conn!(context, api_actor);
    let project = readable_project(conn, context, &path_params.project, api_actor)?;
    let version: VersionRow = schema::version::table
        .filter(schema::version::project_id.eq(project.id))
        .filter(schema::version::uuid.eq(path_params.version))
        .select((
            schema::version::id,
            schema::version::uuid,
            schema::version::number,
            schema::version::hash,
        ))
        .first(conn)
        .map_err(resource_not_found_err!(
            Version,
            (&project, path_params.version)
        ))?;
    let version_ids = [version.0];
    let rows = report_rows(conn, &version_ids).map_err(resource_not_found_err!(
        Report,
        (&project, path_params.version)
    ))?;
    let lines = report_lines(conn, &version_ids).map_err(resource_not_found_err!(
        Metric,
        (&project, path_params.version)
    ))?;
    let alerts = report_alerts(conn, &version_ids).map_err(resource_not_found_err!(
        Alert,
        (&project, path_params.version)
    ))?;
    // A run is a version with reports.
    let facts = RunFacts::new(&rows, &lines, &alerts).ok_or_else(|| {
        resource_not_found_err!(Version, (&project, path_params.version))(
            diesel::result::Error::NotFound,
        )
    })?;
    let testbeds = rows
        .iter()
        .map(|row| row.testbed.uuid)
        .collect::<HashSet<_>>()
        .len();
    // The heading's steps and the run's alerts belong to its first page.
    let (previous, next, alerting) = if query_params.page.unwrap_or(1) <= 1 {
        let (previous, next) = neighbors(
            conn,
            project.id,
            facts.branch.uuid,
            version.0,
            facts.start_time.into(),
        )
        .map_err(resource_not_found_err!(
            Version,
            (&project, path_params.version)
        ))?;
        let alerting = alerting(conn, context, project.id, version.0, &facts, &rows)?;
        (previous, next, alerting)
    } else {
        (None, None, None)
    };
    let (offset, page) = page_of(&rows, query_params.page, query_params.per_page);
    let reports = page
        .iter()
        .enumerate()
        .map(|(index, row)| JsonConsoleRunReport {
            order: to_u32(offset + index + 1),
            uuid: row.uuid,
            testbed: row.testbed.clone(),
            adapter: row.adapter,
            start_time: row.start_time.into(),
            end_time: row.end_time.into(),
            lines: lines.get(&row.id).copied().unwrap_or_default(),
            alerts: alerts.get(&row.id).copied().unwrap_or_default(),
            author: row.author.as_ref().map(Author::json),
        })
        .collect();
    let (_, uuid, number, hash) = version;
    Ok(JsonConsoleRun {
        uuid,
        number,
        hash,
        branch: facts.branch,
        start_time: facts.start_time,
        end_time: facts.end_time,
        total: to_u32(rows.len()),
        testbeds: to_u32(testbeds),
        lines: facts.lines,
        alerts: facts.alerts,
        previous,
        next,
        alerting,
        reports,
    })
}

/// The alerts a run's reports raised, read only for a run whose facts count any.
fn alerting(
    conn: &mut DbConnection,
    context: &ApiContext,
    project_id: ProjectId,
    version_id: VersionId,
    facts: &RunFacts,
    rows: &[ReportRow],
) -> Result<Option<JsonConsoleAlerts>, HttpError> {
    if facts.alerts.total == 0 {
        return Ok(None);
    }
    let in_order = rows
        .iter()
        .map(|row| (row.id, row.uuid))
        .collect::<Vec<_>>();
    run_alerts(
        conn,
        project_id,
        context.clock.now().into(),
        version_id,
        &in_order,
    )
    .map(Some)
}

/// How far either side of a run's first start its Previous and Next are first looked for.
const NEIGHBOR_WINDOW: Duration = Duration::from_hours(4 * 7 * 24);

/// The runs before and after a run on its branch, in the runs list's order. The runs whose
/// first start is within the window either side are read first, and a side reads further only
/// when none of them falls on it.
fn neighbors(
    conn: &mut DbConnection,
    project_id: ProjectId,
    branch: BranchUuid,
    version_id: VersionId,
    first_start: DateTime,
) -> diesel::QueryResult<(Option<JsonConsoleRunLink>, Option<JsonConsoleRunLink>)> {
    let from = before(first_start, NEIGHBOR_WINDOW).unwrap_or(first_start);
    let to = after(first_start, NEIGHBOR_WINDOW).unwrap_or(first_start);
    let run = (version_id, first_start);
    let near = branch_keys(conn, project_id, branch, Some(from), Some(to))?;
    let (mut older, mut newer) = beside(&near, run);
    older = older.filter(|(_, start)| start.timestamp() >= from.timestamp());
    newer = newer.filter(|(_, start)| start.timestamp() <= to.timestamp());
    if older.is_none() {
        older = beside(&branch_keys(conn, project_id, branch, None, None)?, run).0;
    }
    // A run that starts after the window has a report that ends after it.
    if newer.is_none() {
        newer = beside(&branch_keys(conn, project_id, branch, Some(to), None)?, run).1;
    }
    let ids = older
        .iter()
        .chain(&newer)
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    let versions = versions(conn, &ids)?;
    let link = |key: Option<RunKey>| {
        let (id, start_time) = key?;
        let (_, uuid, number, hash) = versions.get(&id)?.clone();
        Some(JsonConsoleRunLink {
            uuid,
            number,
            hash,
            start_time: start_time.into(),
        })
    };
    Ok((link(older), link(newer)))
}

fn after(time: DateTime, duration: Duration) -> Option<DateTime> {
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    DateTime::try_from(time.timestamp().checked_add(seconds)?).ok()
}

/// The runs on a branch and their first report's start, of the runs with a report that ends at
/// or after `ends_after` and starts at or before `starts_before`, each read over all of its
/// visible reports so its first start is its own.
fn branch_keys(
    conn: &mut DbConnection,
    project_id: ProjectId,
    branch: BranchUuid,
    ends_after: Option<DateTime>,
    starts_before: Option<DateTime>,
) -> diesel::QueryResult<Vec<RunKey>> {
    let branch = branch.to_string();
    let keys = if ends_after.is_none() && starts_before.is_none() {
        schema::report::table
            .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
            .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
            .inner_join(
                schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)),
            )
            .filter(schema::branch::uuid.eq(&branch))
            .filter(visible())
            .group_by(schema::report::version_id)
            .select((schema::report::version_id, min(schema::report::start_time)))
            .load::<(VersionId, Option<DateTime>)>(conn)?
    } else {
        let near_report = diesel::alias!(schema::report as near_report);
        let mut near = near_report
            .filter(near_report.field(schema::report::project_id).eq(project_id))
            .select(near_report.field(schema::report::version_id))
            .into_boxed();
        if let Some(time) = ends_after {
            near = near.filter(near_report.field(schema::report::end_time).ge(time));
        }
        if let Some(time) = starts_before {
            near = near.filter(near_report.field(schema::report::start_time).le(time));
        }
        // The `+ 0` reads each candidate by its version rather than every report of the branch.
        schema::report::table
            .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id + 0)))
            .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
            .inner_join(
                schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)),
            )
            .filter(schema::branch::uuid.eq(&branch))
            .filter(visible())
            .filter(schema::report::version_id.eq_any(near))
            .group_by(schema::report::version_id)
            .select((schema::report::version_id, min(schema::report::start_time)))
            .load::<(VersionId, Option<DateTime>)>(conn)?
    };
    Ok(keys
        .into_iter()
        .filter_map(|(version_id, start_time)| Some((version_id, start_time?)))
        .collect())
}

/// The runs either side of a run in the runs list's order, the one before and then the one
/// after, whether or not the keys hold the run itself.
fn beside(keys: &[RunKey], run: RunKey) -> (Option<RunKey>, Option<RunKey>) {
    let order = |&(version_id, start_time): &RunKey| (start_time.timestamp(), version_id);
    let own = order(&run);
    (
        keys.iter()
            .filter(|key| order(key) < own)
            .max_by_key(|key| order(key))
            .copied(),
        keys.iter()
            .filter(|key| order(key) > own)
            .min_by_key(|key| order(key))
            .copied(),
    )
}

/// A run's version and its first report's start, which order the runs list.
type RunKey = (VersionId, DateTime);
