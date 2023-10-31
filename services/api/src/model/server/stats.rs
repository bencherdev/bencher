use bencher_json::{
    system::server::{JsonCohort, JsonCohortAvg},
    DateTime, JsonServerStats,
};
use diesel::{dsl::count, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::organization::QueryOrganization,
    schema,
};

use super::QueryServer;

const THIS_WEEK: i64 = 7 * 24 * 60 * 60;
const THIS_MONTH: i64 = THIS_WEEK * 4;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
pub fn get_stats(
    conn: &mut DbConnection,
    query_server: QueryServer,
) -> Result<JsonServerStats, HttpError> {
    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

    // organizations
    let organizations = schema::organization::table
        .load::<QueryOrganization>(conn)
        .map_err(resource_not_found_err!(Organization))?
        .into_iter()
        .map(QueryOrganization::into_json)
        .collect();

    // users
    let weekly_users = schema::user::table
        .filter(schema::user::created.ge(this_week))
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let monthly_users = schema::user::table
        .filter(schema::user::created.ge(this_month))
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let total_users = schema::user::table
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let users_cohort = JsonCohort {
        week: weekly_users as u64,
        month: monthly_users as u64,
        total: total_users as u64,
    };

    // projects
    let weekly_projects = schema::project::table
        .filter(schema::project::created.ge(this_week))
        .select(count(schema::project::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let monthly_projects = schema::project::table
        .filter(schema::project::created.ge(this_month))
        .select(count(schema::project::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let total_projects = schema::project::table
        .select(count(schema::project::id))
        .first::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let projects_cohort = JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    };

    // reports and median reports per project
    let mut weekly_reports = schema::report::table
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let weekly_active_projects = weekly_reports.len();
    let weekly_reports_total: i64 = weekly_reports.iter().sum();
    let weekly_reports_per_project = median(&mut weekly_reports);

    let mut monthly_reports = schema::report::table
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let monthly_active_projects = monthly_reports.len();
    let monthly_reports_total: i64 = monthly_reports.iter().sum();
    let monthly_reports_per_project = median(&mut monthly_reports);

    let mut total_reports = schema::report::table
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let total_active_projects = total_reports.len();
    let total_reports_total: i64 = total_reports.iter().sum();
    let total_reports_per_project = median(&mut total_reports);

    let active_projects_cohort = JsonCohort {
        week: weekly_active_projects as u64,
        month: monthly_active_projects as u64,
        total: total_active_projects as u64,
    };

    let reports_cohort = JsonCohort {
        week: weekly_reports_total as u64,
        month: monthly_reports_total as u64,
        total: total_reports_total as u64,
    };

    let reports_per_project_cohort = JsonCohortAvg {
        week: weekly_reports_per_project,
        month: monthly_reports_per_project,
        total: total_reports_per_project,
    };

    // metrics and median metrics per report
    let mut weekly_metrics = schema::metric::table
        .inner_join(
            schema::perf::table
                .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                .inner_join(schema::report::table),
        )
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Metric))?;
    let weekly_metrics_total: i64 = weekly_metrics.iter().sum();
    let weekly_metrics_per_project = median(&mut weekly_metrics);

    let mut monthly_metrics = schema::metric::table
        .inner_join(
            schema::perf::table
                .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                .inner_join(schema::report::table),
        )
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Metric))?;
    let monthly_metrics_total: i64 = monthly_metrics.iter().sum();
    let monthly_metrics_per_project = median(&mut monthly_metrics);

    let mut total_metrics = schema::metric::table
        .inner_join(
            schema::perf::table
                .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                .inner_join(schema::report::table),
        )
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Metric))?;
    let total_metrics_total: i64 = total_metrics.iter().sum();
    let total_metrics_per_project = median(&mut total_metrics);

    let metrics_cohort = JsonCohort {
        week: weekly_metrics_total as u64,
        month: monthly_metrics_total as u64,
        total: total_metrics_total as u64,
    };

    let metrics_per_report_cohort = JsonCohortAvg {
        week: weekly_metrics_per_project,
        month: monthly_metrics_per_project,
        total: total_metrics_per_project,
    };

    Ok(JsonServerStats {
        server: query_server.into_json(),
        organizations,
        timestamp: now,
        users: users_cohort,
        projects: projects_cohort,
        active_projects: active_projects_cohort,
        reports: reports_cohort,
        reports_per_project: reports_per_project_cohort,
        metrics: metrics_cohort,
        metrics_per_report: metrics_per_report_cohort,
    })
}

#[allow(
    clippy::integer_division,
    clippy::cast_precision_loss,
    clippy::indexing_slicing
)]
fn median(array: &mut Vec<i64>) -> f64 {
    if array.is_empty() {
        return 0.0;
    }

    array.sort_unstable();

    let size = array.len();
    if (size % 2) == 0 {
        let left = size / 2 - 1;
        let right = size / 2;
        (array[left] as f64 + array[right] as f64) / 2.0
    } else {
        array[size / 2] as f64
    }
}
