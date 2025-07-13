use bencher_json::{
    DateTime, JsonOrganizations, JsonServerStats, JsonUsers,
    system::server::{JsonCohort, JsonCohortAvg, JsonTopCohort, JsonTopProject, JsonTopProjects},
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
    dsl::count,
};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    connection_lock,
    context::DbConnection,
    error::resource_not_found_err,
    model::{organization::QueryOrganization, project::QueryProject, user::QueryUser},
    schema,
};

use super::QueryServer;

const THIS_WEEK: i64 = 7 * 24 * 60 * 60;
const THIS_MONTH: i64 = THIS_WEEK * 4;
const TOP_PROJECTS: usize = 10;

#[expect(clippy::cast_sign_loss, clippy::too_many_lines)]
pub async fn get_stats(
    db_connection: &Mutex<DbConnection>,
    query_server: QueryServer,
    is_bencher_cloud: bool,
) -> Result<JsonServerStats, HttpError> {
    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

    let organizations = get_organizations(db_connection, is_bencher_cloud).await?;
    let admins = get_admins(db_connection, is_bencher_cloud).await?;
    let users_cohort = get_users(db_connection, this_week, this_month).await?;
    let projects_cohort = get_projects(db_connection, this_week, this_month).await?;
    let unclaimed_projects_cohort =
        get_unclaimed_projects(db_connection, this_week, this_month).await?;
    let claimed_projects_cohort =
        get_claimed_projects(db_connection, this_week, this_month).await?;

    // reports and median reports per project
    let mut weekly_reports = schema::report::table
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Report))?;
    let weekly_active_projects = weekly_reports.len();
    let weekly_reports_total: i64 = weekly_reports.iter().sum();
    let weekly_reports_per_project = median(&mut weekly_reports);

    let mut monthly_reports = schema::report::table
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Report))?;
    let monthly_active_projects = monthly_reports.len();
    let monthly_reports_total: i64 = monthly_reports.iter().sum();
    let monthly_reports_per_project = median(&mut monthly_reports);

    let mut total_reports = schema::report::table
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(connection_lock!(db_connection))
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
        .inner_join(schema::report_benchmark::table.inner_join(schema::report::table))
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Metric))?;
    let weekly_metrics_total: i64 = weekly_metrics.iter().sum();
    let weekly_metrics_per_project = median(&mut weekly_metrics);

    let mut monthly_metrics = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::report::table))
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Metric))?;
    let monthly_metrics_total: i64 = monthly_metrics.iter().sum();
    let monthly_metrics_per_project = median(&mut monthly_metrics);

    let mut total_metrics = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::report::table))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .load::<i64>(connection_lock!(db_connection))
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

    // top projects
    let weekly_project_metrics = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::report::table.inner_join(schema::project::table)),
        )
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::project::id)
        .select((QueryProject::as_select(), count(schema::metric::id)))
        .load::<(QueryProject, i64)>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Project))?;
    let weekly_project_metrics = top_projects(weekly_project_metrics, weekly_metrics_total);

    let monthly_project_metrics = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::report::table.inner_join(schema::project::table)),
        )
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::project::id)
        .select((QueryProject::as_select(), count(schema::metric::id)))
        .load::<(QueryProject, i64)>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Project))?;
    let monthly_project_metrics = top_projects(monthly_project_metrics, monthly_metrics_total);

    let total_project_metrics = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::report::table.inner_join(schema::project::table)),
        )
        .group_by(schema::project::id)
        .select((QueryProject::as_select(), count(schema::metric::id)))
        .load::<(QueryProject, i64)>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(Metric))?;
    let total_project_metrics = top_projects(total_project_metrics, total_metrics_total);

    let top_projects_cohort = JsonTopCohort {
        week: weekly_project_metrics,
        month: monthly_project_metrics,
        total: total_project_metrics,
    };

    Ok(JsonServerStats {
        server: query_server.into_json(),
        timestamp: now,
        organizations,
        admins,
        users: Some(users_cohort),
        projects: Some(projects_cohort),
        unclaimed_projects: Some(unclaimed_projects_cohort),
        claimed_projects: Some(claimed_projects_cohort),
        active_projects: Some(active_projects_cohort),
        reports: Some(reports_cohort),
        reports_per_project: Some(reports_per_project_cohort),
        metrics: Some(metrics_cohort),
        metrics_per_report: Some(metrics_per_report_cohort),
        top_projects: Some(top_projects_cohort),
    })
}

async fn get_organizations(
    db_connection: &Mutex<DbConnection>,
    is_bencher_cloud: bool,
) -> Result<Option<JsonOrganizations>, HttpError> {
    Ok(if is_bencher_cloud {
        None
    } else {
        Some(connection_lock!(db_connection, |conn| {
            schema::organization::table
                .load::<QueryOrganization>(conn)
                .map_err(resource_not_found_err!(Organization))?
                .into_iter()
                .map(|org| org.into_json(conn))
                .collect()
        }))
    })
}

async fn get_admins(
    db_connection: &Mutex<DbConnection>,
    is_bencher_cloud: bool,
) -> Result<Option<JsonUsers>, HttpError> {
    Ok(if is_bencher_cloud {
        None
    } else {
        Some(connection_lock!(db_connection, |conn| {
            schema::user::table
                .filter(schema::user::admin.eq(true))
                .load::<QueryUser>(conn)
                .map_err(resource_not_found_err!(User))?
                .into_iter()
                .map(QueryUser::into_json)
                .collect()
        }))
    })
}

async fn get_users(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_users = schema::user::table
        .filter(schema::user::created.ge(this_week))
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let monthly_users = schema::user::table
        .filter(schema::user::created.ge(this_month))
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let total_users = schema::user::table
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    Ok(JsonCohort {
        week: weekly_users as u64,
        month: monthly_users as u64,
        total: total_users as u64,
    })
}

async fn get_projects(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = schema::project::table
        .filter(schema::project::created.ge(this_week))
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let monthly_projects = schema::project::table
        .filter(schema::project::created.ge(this_month))
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let total_projects = schema::project::table
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    })
}

async fn get_unclaimed_projects(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_null())
        .filter(schema::project::created.ge(this_week))
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let monthly_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_null())
        .filter(schema::project::created.ge(this_month))
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let total_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_null())
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    })
}

async fn get_claimed_projects(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_not_null())
        .filter(schema::project::created.ge(this_week))
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let monthly_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_not_null())
        .filter(schema::project::created.ge(this_month))
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    let total_projects = schema::project::table
        .inner_join(schema::organization::table.left_join(schema::organization_role::table))
        .filter(schema::organization_role::id.is_not_null())
        .group_by(schema::project::id)
        .count()
        .get_result::<i64>(connection_lock!(db_connection))
        .map_err(resource_not_found_err!(User))?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
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

#[expect(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn top_projects(mut project_metrics: Vec<(QueryProject, i64)>, total: i64) -> JsonTopProjects {
    project_metrics.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    project_metrics.reverse();
    if project_metrics.len() > TOP_PROJECTS {
        project_metrics.truncate(TOP_PROJECTS);
    }
    project_metrics
        .into_iter()
        .map(|(project, metrics)| JsonTopProject {
            name: project.name,
            uuid: project.uuid,
            metrics: metrics as u64,
            percentage: metrics as f64 / total as f64,
        })
        .collect()
}
