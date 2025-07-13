use bencher_json::{
    DateTime, JsonOrganizations, JsonServerStats, JsonUsers,
    organization::claim,
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

    // projects
    let projects_cohort =
        get_projects_cohort(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_projects_cohort = get_projects_cohort(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_projects_cohort =
        get_projects_cohort(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    // reports and median reports per project
    let reports_cohorts =
        get_reports_cohorts(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_reports_cohorts = get_reports_cohorts(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_reports_cohorts =
        get_reports_cohorts(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    // metrics and median metrics per report
    let metrics_cohorts =
        get_metrics_cohorts(db_connection, this_week, this_month, ProjectState::All).await?;
    let unclaimed_metrics_cohorts = get_metrics_cohorts(
        db_connection,
        this_week,
        this_month,
        ProjectState::Unclaimed,
    )
    .await?;
    let claimed_metrics_cohorts =
        get_metrics_cohorts(db_connection, this_week, this_month, ProjectState::Claimed).await?;

    Ok(JsonServerStats {
        server: query_server.into_json(),
        timestamp: now,
        organizations,
        admins,
        users: Some(users_cohort),
        projects: Some(projects_cohort),
        unclaimed_projects: Some(unclaimed_projects_cohort),
        claimed_projects: Some(claimed_projects_cohort),
        active_projects: Some(reports_cohorts.active_projects_cohort),
        active_unclaimed_projects: Some(unclaimed_reports_cohorts.active_projects_cohort),
        active_claimed_projects: Some(claimed_reports_cohorts.active_projects_cohort),
        reports: Some(reports_cohorts.reports_cohort),
        unclaimed_reports: Some(unclaimed_reports_cohorts.reports_cohort),
        claimed_reports: Some(claimed_reports_cohorts.reports_cohort),
        reports_per_project: Some(reports_cohorts.reports_per_project_cohort),
        reports_per_unclaimed_project: Some(unclaimed_reports_cohorts.reports_per_project_cohort),
        reports_per_claimed_project: Some(claimed_reports_cohorts.reports_per_project_cohort),
        metrics: Some(metrics_cohorts.metrics_cohort),
        unclaimed_metrics: Some(unclaimed_metrics_cohorts.metrics_cohort),
        claimed_metrics: Some(claimed_metrics_cohorts.metrics_cohort),
        metrics_per_report: Some(metrics_cohorts.metrics_per_report_cohort),
        metrics_per_unclaimed_report: Some(unclaimed_metrics_cohorts.metrics_per_report_cohort),
        metrics_per_claimed_report: Some(claimed_metrics_cohorts.metrics_per_report_cohort),
        top_projects: Some(metrics_cohorts.top_projects_cohort),
        top_unclaimed_projects: Some(unclaimed_metrics_cohorts.top_projects_cohort),
        top_claimed_projects: Some(claimed_metrics_cohorts.top_projects_cohort),
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

enum ProjectState {
    All,
    Unclaimed,
    Claimed,
}

async fn get_projects_cohort(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
    state: ProjectState,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = get_project_count(db_connection, Some(this_week), &state).await?;
    let monthly_projects = get_project_count(db_connection, Some(this_month), &state).await?;
    let total_projects = get_project_count(db_connection, None, &state).await?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    })
}

async fn get_project_count(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<i64, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::project::table.into_boxed();
            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }
            query
                .count()
                .get_result::<i64>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Project))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::project::table
                .inner_join(schema::organization::table.left_join(schema::organization_role::table))
                .select(diesel::dsl::count_distinct(schema::project::id))
                .into_boxed();

            query = match state {
                ProjectState::All => unreachable!(),
                ProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                ProjectState::Claimed => query.filter(schema::organization_role::id.is_not_null()),
            };

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .first::<i64>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Project))
        },
    }
}

struct ReportsCohorts {
    active_projects_cohort: JsonCohort,
    reports_cohort: JsonCohort,
    reports_per_project_cohort: JsonCohortAvg,
}

async fn get_reports_cohorts(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
    state: ProjectState,
) -> Result<ReportsCohorts, HttpError> {
    let mut weekly_reports = get_reports_by_project(db_connection, Some(this_week), &state).await?;
    let weekly_active_projects = weekly_reports.len();
    let weekly_reports_total: i64 = weekly_reports.iter().sum();
    let weekly_reports_per_project = median(&mut weekly_reports);

    let mut monthly_reports =
        get_reports_by_project(db_connection, Some(this_month), &state).await?;
    let monthly_active_projects = monthly_reports.len();
    let monthly_reports_total: i64 = monthly_reports.iter().sum();
    let monthly_reports_per_project = median(&mut monthly_reports);

    let mut total_reports = get_reports_by_project(db_connection, None, &state).await?;
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

    Ok(ReportsCohorts {
        active_projects_cohort,
        reports_cohort,
        reports_per_project_cohort,
    })
}

async fn get_reports_by_project(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<Vec<i64>, HttpError> {
    let mut query = schema::report::table
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .into_boxed();

    if let Some(since) = since {
        query = query.filter(schema::report::created.ge(since));
    }

    match state {
        ProjectState::All => query
            .load::<i64>(connection_lock!(db_connection))
            .map_err(resource_not_found_err!(Report)),
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::report::table
                .inner_join(schema::project::table.inner_join(
                    schema::organization::table.left_join(schema::organization_role::table),
                ))
                .group_by(schema::report::project_id)
                .select(count(schema::report::id))
                .into_boxed();

            query = match state {
                ProjectState::All => unreachable!(),
                ProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                ProjectState::Claimed => query.filter(schema::organization_role::id.is_not_null()),
            };

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i64>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Report))
        },
    }
}

struct MetricsCohorts {
    metrics_cohort: JsonCohort,
    metrics_per_report_cohort: JsonCohortAvg,
    top_projects_cohort: JsonTopCohort,
}

async fn get_metrics_cohorts(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
    state: ProjectState,
) -> Result<MetricsCohorts, HttpError> {
    let mut weekly_metrics = get_metrics_by_report(db_connection, Some(this_week), &state).await?;
    let weekly_metrics_total: i64 = weekly_metrics.iter().sum();
    let weekly_metrics_per_project = median(&mut weekly_metrics);
    let weekly_top_projects = get_top_projects(db_connection, Some(this_week), &state).await?;

    let mut monthly_metrics =
        get_metrics_by_report(db_connection, Some(this_month), &state).await?;
    let monthly_metrics_total: i64 = monthly_metrics.iter().sum();
    let monthly_metrics_per_project = median(&mut monthly_metrics);
    let monthly_top_projects = get_top_projects(db_connection, Some(this_month), &state).await?;

    let mut total_metrics = get_metrics_by_report(db_connection, None, &state).await?;
    let total_metrics_total: i64 = total_metrics.iter().sum();
    let total_metrics_per_project = median(&mut total_metrics);
    let total_top_projects = get_top_projects(db_connection, None, &state).await?;

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

    let top_projects_cohort = JsonTopCohort {
        week: top_projects(weekly_top_projects, weekly_metrics_total),
        month: top_projects(monthly_top_projects, monthly_metrics_total),
        total: top_projects(total_top_projects, total_metrics_total),
    };

    Ok(MetricsCohorts {
        metrics_cohort,
        metrics_per_report_cohort,
        top_projects_cohort,
    })
}

async fn get_metrics_by_report(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<Vec<i64>, HttpError> {
    let mut query = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::report::table))
        .group_by(schema::report::id)
        .select(count(schema::metric::id))
        .into_boxed();

    if let Some(since) = since {
        query = query.filter(schema::report::created.ge(since));
    }

    match state {
        ProjectState::All => query
            .load::<i64>(connection_lock!(db_connection))
            .map_err(resource_not_found_err!(Metric)),
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                ))
                .group_by(schema::report::id)
                .select(count(schema::metric::id))
                .into_boxed();

            query = match state {
                ProjectState::All => unreachable!(),
                ProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                ProjectState::Claimed => query.filter(schema::organization_role::id.is_not_null()),
            };

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i64>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Metric))
        },
    }
}

async fn get_top_projects(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<Vec<(QueryProject, i64)>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::metric::table
                .inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::report::table.inner_join(schema::project::table)),
                )
                .group_by(schema::project::id)
                .select((QueryProject::as_select(), count(schema::metric::id)))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<(QueryProject, i64)>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Project))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                ))
                .group_by(schema::project::id)
                .select((QueryProject::as_select(), count(schema::metric::id)))
                .into_boxed();

            query = match state {
                ProjectState::All => unreachable!(),
                ProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                ProjectState::Claimed => query.filter(schema::organization_role::id.is_not_null()),
            };

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<(QueryProject, i64)>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Project))
        },
    }
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
