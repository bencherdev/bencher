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

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

    // median reports per project
    let mut weekly_reports = schema::report::table
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let weekly_reports = padded_median(total_projects as usize, &mut weekly_reports);

    let mut monthly_reports = schema::report::table
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let monthly_reports = padded_median(total_projects as usize, &mut monthly_reports);

    let mut total_reports = schema::report::table
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let total_reports = padded_median(total_projects as usize, &mut total_reports);

    let reports_cohort = JsonCohortAvg {
        week: weekly_reports,
        month: monthly_reports,
        total: total_reports,
    };

    Ok(JsonServerStats {
        timestamp: now,
        users: users_cohort,
        projects: projects_cohort,
        reports: reports_cohort,
    })
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
