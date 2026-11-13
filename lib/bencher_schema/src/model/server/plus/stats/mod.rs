use bencher_json::{
    DateTime, JsonServerStats,
    system::server::{JsonRunnerStats, JsonRunnerStatsCohort},
};
use dropshot::HttpError;
use slog::Logger;

use crate::context::DbConnection;

use super::QueryServer;

mod job_stats;
mod metrics_stats;
mod organization_stats;
mod projects_stats;
mod reports_stats;
mod users_stats;

use job_stats::JobStats;
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
    Plus,
}

pub(super) fn get_stats(
    log: &Logger,
    conn: &mut DbConnection,
    query_server: QueryServer,
    is_bencher_cloud: bool,
) -> Result<JsonServerStats, HttpError> {
    slog::info!(log, "Collecting server stats");

    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

    let organizations_stats = OrganizationStats::new(conn, is_bencher_cloud)?;

    // users
    let users_stats = UsersStats::new(conn, this_week, this_month, is_bencher_cloud)?;

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

    // job duration and median job duration per report
    let job_stats = JobStats::new(conn, this_week, this_month, ProjectState::All)?;
    let unclaimed_job_stats = JobStats::new(conn, this_week, this_month, ProjectState::Unclaimed)?;
    let claimed_job_stats = JobStats::new(conn, this_week, this_month, ProjectState::Claimed)?;
    let plus_job_stats = JobStats::new(conn, this_week, this_month, ProjectState::Plus)?;

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
        runner: Some(JsonRunnerStats {
            seconds: JsonRunnerStatsCohort {
                total: job_stats.seconds,
                unclaimed: unclaimed_job_stats.seconds,
                claimed: claimed_job_stats.seconds,
                plus: plus_job_stats.seconds,
            },
            seconds_per_report: JsonRunnerStatsCohort {
                total: job_stats.seconds_per_report,
                unclaimed: unclaimed_job_stats.seconds_per_report,
                claimed: claimed_job_stats.seconds_per_report,
                plus: plus_job_stats.seconds_per_report,
            },
            top_projects: JsonRunnerStatsCohort {
                total: job_stats.top_projects,
                unclaimed: unclaimed_job_stats.top_projects,
                claimed: claimed_job_stats.top_projects,
                plus: plus_job_stats.top_projects,
            },
        }),
    })
}

#[expect(
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::integer_division,
    reason = "median math on small sorted arrays"
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

#[cfg(test)]
mod tests {
    use bencher_json::DateTime;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use crate::{
        context::DbConnection,
        macros::sql::last_insert_rowid,
        model::organization::OrganizationId,
        schema,
        test_util::{
            create_branch_with_head, create_report, create_testbed, create_version, setup_test_db,
        },
    };

    use super::{JobStats, MetricsStats, ProjectState, ProjectsStats, ReportsStats};

    /// An organization `n` with a licensed plan row when `licensed`, or else a metered one, and
    /// one project with one report of `metrics` metrics and a job that ran `seconds`.
    fn seed(conn: &mut DbConnection, n: u8, licensed: bool, metrics: i32, seconds: i32) {
        let uuid = |kind: u8| format!("00000000-0000-4000-8000-0000000{kind:02}{n:03}");
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(uuid(1)),
                schema::organization::name.eq(format!("Organization {n}")),
                schema::organization::slug.eq(format!("organization-{n}")),
                schema::organization::created.eq(DateTime::TEST),
                schema::organization::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
            .unwrap();
        let organization_id: OrganizationId = diesel::select(last_insert_rowid())
            .get_result(conn)
            .unwrap();
        diesel::insert_into(schema::project::table)
            .values((
                schema::project::uuid.eq(uuid(2)),
                schema::project::organization_id.eq(organization_id),
                schema::project::name.eq(format!("Project {n}")),
                schema::project::slug.eq(format!("project-{n}")),
                schema::project::visibility.eq(0),
                schema::project::created.eq(DateTime::TEST),
                schema::project::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
            .unwrap();
        let project_id = schema::project::table
            .filter(schema::project::uuid.eq(uuid(2)))
            .select(schema::project::id)
            .first(conn)
            .unwrap();
        let head_id =
            create_branch_with_head(conn, project_id, &uuid(3), "main", "main", &uuid(4)).head_id;
        let version_id = create_version(conn, project_id, &uuid(5), 1, None);
        let testbed_id = create_testbed(conn, project_id, &uuid(6), "localhost", "localhost");
        let report_id = create_report(conn, &uuid(7), project_id, head_id, version_id, testbed_id);
        diesel::insert_into(schema::metric_count_by_report::table)
            .values((
                schema::metric_count_by_report::report_id.eq(report_id),
                schema::metric_count_by_report::metric_count.eq(metrics),
            ))
            .execute(conn)
            .unwrap();
        diesel::insert_into(schema::job_duration_by_report::table)
            .values((
                schema::job_duration_by_report::report_id.eq(report_id),
                schema::job_duration_by_report::job_duration.eq(seconds),
            ))
            .execute(conn)
            .unwrap();
        let plan = (
            schema::plan::organization_id.eq(organization_id),
            schema::plan::created.eq(DateTime::TEST),
            schema::plan::modified.eq(DateTime::TEST),
        );
        if licensed {
            diesel::insert_into(schema::plan::table)
                .values((
                    plan,
                    schema::plan::licensed_plan.eq("sub_licensed"),
                    schema::plan::license.eq("header.claims.signature"),
                ))
                .execute(conn)
                .unwrap();
        } else {
            diesel::insert_into(schema::plan::table)
                .values((plan, schema::plan::metered_plan.eq("sub_metered")))
                .execute(conn)
                .unwrap();
        }
    }

    /// A count that a stat of paid projects reads.
    type Count = fn(&mut DbConnection) -> u64;

    // Fails if any stat of paid projects counts the organization that holds only a licensed plan
    // row, the Bencher Cloud organization that buys a Self-Hosted license, or leaves out the one
    // with a Bencher Cloud plan row.
    #[test]
    fn paid_stats_count_only_bencher_cloud_plans() {
        let mut conn = setup_test_db();
        seed(&mut conn, 1, false, 3, 60);
        seed(&mut conn, 2, true, 5, 120);

        let stats: [(&str, Count, u64); 6] = [
            (
                "projects",
                |conn| {
                    ProjectsStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .projects
                        .total
                },
                1,
            ),
            (
                "reports",
                |conn| {
                    ReportsStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .reports
                        .total
                },
                1,
            ),
            (
                "metrics",
                |conn| {
                    MetricsStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .metrics
                        .total
                },
                3,
            ),
            (
                "top projects by metrics",
                |conn| {
                    let top = MetricsStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .top_projects
                        .total;
                    top.iter().map(|project| project.metrics).sum()
                },
                3,
            ),
            (
                "runner seconds",
                |conn| {
                    JobStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .seconds
                        .total
                },
                60,
            ),
            (
                "top projects by runner seconds",
                |conn| {
                    let top = JobStats::new(conn, 0, 0, ProjectState::Plus)
                        .unwrap()
                        .top_projects
                        .total;
                    top.iter().map(|project| project.seconds).sum()
                },
                60,
            ),
        ];
        for (stat, count, expected) in stats {
            assert_eq!(count(&mut conn), expected, "{stat}");
        }
    }
}
