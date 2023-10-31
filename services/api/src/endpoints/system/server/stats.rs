#![cfg(feature = "plus")]

use bencher_json::{
    system::stats::{JsonCohort, JsonCohortAvg},
    DateTime, JsonServerStats,
};
use diesel::{
    dsl::{avg, count},
    sql_types::{BigInt, Double, Integer, Numeric},
    ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
};
use dropshot::{endpoint, HttpError, RequestContext};

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::resource_not_found_err,
    model::{
        project::ProjectId,
        user::{admin::AdminUser, auth::BearerToken},
    },
    schema,
};

const THIS_WEEK: i64 = 7 * 24 * 60 * 60;
const THIS_MONTH: i64 = THIS_WEEK * 4;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/server/stats",
    tags = ["server", "stats"]
}]
pub async fn server_stats_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseOk<JsonServerStats>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context()).await?;
    Ok(Get::auth_response_ok(json))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
async fn get_one_inner(context: &ApiContext) -> Result<JsonServerStats, HttpError> {
    let conn = &mut *context.conn().await;

    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

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

#[allow(
    clippy::integer_division,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::indexing_slicing
)]
fn padded_median(total: usize, array: &mut Vec<i64>) -> f64 {
    if total == 0 {
        return 0.0;
    }

    array.sort_unstable();

    let zeros = total - array.len();
    if total % 2 == 0 {
        let left = total / 2 - 1;
        let right = total / 2;
        let (left_index, right_index) = match (left.checked_sub(zeros), right.checked_sub(zeros)) {
            (Some(left_index), Some(right_index)) => (left_index, right_index),
            (None, Some(right_index)) => (0, right_index),
            (Some(left_index), None) => (left_index, 0),
            (None, None) => return 0.0,
        };
        (array[left_index] as f64 + array[right_index] as f64) / 2.0
    } else {
        let middle = total / 2;
        match middle.checked_sub(zeros) {
            Some(middle_index) => array[middle_index] as f64,
            None => 0.0,
        }
    }
}
