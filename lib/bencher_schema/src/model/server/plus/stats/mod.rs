use bencher_json::{DateTime, JsonServerStats};
use dropshot::HttpError;
use slog::Logger;

use crate::context::DbConnection;

use super::QueryServer;

mod metrics_stats;
mod organization_stats;
mod projects_stats;
mod reports_stats;
mod users_stats;

use metrics_stats::MetricsStats;
use organization_stats::OrganizationStats;
use projects_stats::ProjectsStats;
use reports_stats::ReportsStats;
use users_stats::UsersStats;

const THIS_WEEK: i64 = 7 * 24 * 60 * 60;
const THIS_MONTH: i64 = THIS_WEEK * 4;
const TOP_PROJECTS: usize = 10;

#[derive(Debug, Clone, Copy)]
enum ProjectState {
    All,
    Unclaimed,
    Claimed,
}

pub(super) fn get_stats(
    log: &Logger,
    conn: &mut DbConnection,
    query_server: QueryServer,
) -> Result<JsonServerStats, HttpError> {
    slog::info!(log, "Collecting server stats");

    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

    let organizations_stats = OrganizationStats::new(conn)?;

    // users
    let users_stats = UsersStats::new(conn, this_week, this_month)?;

    // projects
    let projects_stats = ProjectsStats::new(conn, this_week, this_month, ProjectState::All)?;

    let unclaimed_projects_stats =
        ProjectsStats::new(conn, this_week, this_month, ProjectState::Unclaimed)?;
    let claimed_projects_stats =
        ProjectsStats::new(conn, this_week, this_month, ProjectState::Claimed)?;

    // reports and median reports per project
    let reports_stats = ReportsStats::new(conn, this_week, this_month, ProjectState::All)?;
    let unclaimed_reports_stats =
        ReportsStats::new(conn, this_week, this_month, ProjectState::Unclaimed)?;
    let claimed_reports_stats =
        ReportsStats::new(conn, this_week, this_month, ProjectState::Claimed)?;

    // metrics and median metrics per report
    let metrics_stats = MetricsStats::new(conn, this_week, this_month, ProjectState::All)?;
    let unclaimed_metrics_stats =
        MetricsStats::new(conn, this_week, this_month, ProjectState::Unclaimed)?;
    let claimed_metrics_stats =
        MetricsStats::new(conn, this_week, this_month, ProjectState::Claimed)?;

    Ok(JsonServerStats {
        server: query_server.into_json(),
        timestamp: now,
        organizations: organizations_stats.organizations,
        admins: users_stats.admins,
        users: Some(users_stats.users),
        projects: Some(projects_stats.projects),
        projects_unclaimed: Some(unclaimed_projects_stats.projects),
        projects_claimed: Some(claimed_projects_stats.projects),
        active_projects: Some(reports_stats.active_projects),
        active_projects_unclaimed: Some(unclaimed_reports_stats.active_projects),
        active_projects_claimed: Some(claimed_reports_stats.active_projects),
        reports: Some(reports_stats.reports),
        reports_unclaimed: Some(unclaimed_reports_stats.reports),
        reports_claimed: Some(claimed_reports_stats.reports),
        reports_per_project: Some(reports_stats.reports_per_project),
        reports_per_project_unclaimed: Some(unclaimed_reports_stats.reports_per_project),
        reports_per_project_claimed: Some(claimed_reports_stats.reports_per_project),
        metrics: Some(metrics_stats.metrics),
        metrics_unclaimed: Some(unclaimed_metrics_stats.metrics),
        metrics_claimed: Some(claimed_metrics_stats.metrics),
        metrics_per_report: Some(metrics_stats.metrics_per_report),
        metrics_per_report_unclaimed: Some(unclaimed_metrics_stats.metrics_per_report),
        metrics_per_report_claimed: Some(claimed_metrics_stats.metrics_per_report),
        top_projects: Some(metrics_stats.top_projects),
        top_projects_unclaimed: Some(unclaimed_metrics_stats.top_projects),
        top_projects_claimed: Some(claimed_metrics_stats.top_projects),
    })
}

#[expect(
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::integer_division
)]
fn median(array: &mut [i64]) -> f64 {
    if array.is_empty() {
        return 0.0;
    }

    array.sort_unstable();

    let size = array.len();
    if size.is_multiple_of(2) {
        let left = size / 2 - 1;
        let right = size / 2;
        f64::midpoint(array[left] as f64, array[right] as f64)
    } else {
        array[size / 2] as f64
    }
}
