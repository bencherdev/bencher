use bencher_json::{DateTime, JsonServerStats};
use dropshot::HttpError;
use tokio::sync::Mutex;

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

enum ProjectState {
    All,
    Unclaimed,
    Claimed,
}

pub async fn get_stats(
    db_connection: &Mutex<DbConnection>,
    query_server: QueryServer,
    is_bencher_cloud: bool,
) -> Result<JsonServerStats, HttpError> {
    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

    let organizations_stats = OrganizationStats::new(db_connection, is_bencher_cloud).await?;

    // users
    let users_stats =
        UsersStats::new(db_connection, this_week, this_month, is_bencher_cloud).await?;

    // projects
    let projects_stats =
        ProjectsStats::new(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_projects_stats = ProjectsStats::new(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_projects_stats =
        ProjectsStats::new(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    // reports and median reports per project
    let reports_stats =
        ReportsStats::new(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_reports_stats = ReportsStats::new(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_reports_stats =
        ReportsStats::new(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    // metrics and median metrics per report
    let metrics_stats =
        MetricsStats::new(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_metrics_stats = MetricsStats::new(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_metrics_stats =
        MetricsStats::new(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    Ok(JsonServerStats {
        server: query_server.into_json(),
        timestamp: now,
        organizations: organizations_stats.organizations,
        admins: users_stats.admins,
        users: Some(users_stats.users),
        projects: Some(projects_stats.projects),
        unclaimed_projects: Some(unclaimed_projects_stats.projects),
        claimed_projects: Some(claimed_projects_stats.projects),
        active_projects: Some(reports_stats.active_projects),
        active_unclaimed_projects: Some(unclaimed_reports_stats.active_projects),
        active_claimed_projects: Some(claimed_reports_stats.active_projects),
        reports: Some(reports_stats.reports),
        unclaimed_reports: Some(unclaimed_reports_stats.reports),
        claimed_reports: Some(claimed_reports_stats.reports),
        reports_per_project: Some(reports_stats.reports_per_project),
        reports_per_unclaimed_project: Some(unclaimed_reports_stats.reports_per_project),
        reports_per_claimed_project: Some(claimed_reports_stats.reports_per_project),
        metrics: Some(metrics_stats.metrics),
        unclaimed_metrics: Some(unclaimed_metrics_stats.metrics),
        claimed_metrics: Some(claimed_metrics_stats.metrics),
        metrics_per_report: Some(metrics_stats.metrics_per_report),
        metrics_per_unclaimed_report: Some(unclaimed_metrics_stats.metrics_per_report),
        metrics_per_claimed_report: Some(claimed_metrics_stats.metrics_per_report),
        top_projects: Some(metrics_stats.top_projects),
        top_unclaimed_projects: Some(unclaimed_metrics_stats.top_projects),
        top_claimed_projects: Some(claimed_metrics_stats.top_projects),
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
    if (size % 2) == 0 {
        let left = size / 2 - 1;
        let right = size / 2;
        f64::midpoint(array[left] as f64, array[right] as f64)
    } else {
        array[size / 2] as f64
    }
}
