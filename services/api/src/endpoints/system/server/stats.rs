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

#[allow(clippy::cast_sign_loss)]
async fn get_one_inner(context: &ApiContext) -> Result<JsonServerStats, HttpError> {
    let conn = &mut *context.conn().await;

    let now = DateTime::now();
    let timestamp = now.timestamp();
    let this_week = timestamp - THIS_WEEK;
    let this_month = timestamp - THIS_MONTH;

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

    let mut weekly_reports = schema::report::table
        .filter(schema::report::created.ge(this_week))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let avg_weekly_reports = median(&mut weekly_reports);

    let mut monthly_reports = schema::report::table
        .filter(schema::report::created.ge(this_month))
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let avg_monthly_reports = median(&mut monthly_reports);

    let mut total_reports = schema::report::table
        .group_by(schema::report::project_id)
        .select(count(schema::report::id))
        .load::<i64>(conn)
        .map_err(resource_not_found_err!(Report))?;
    let avg_total_reports = median(&mut total_reports);

    let reports_cohort = JsonCohortAvg {
        week: avg_weekly_reports,
        month: avg_monthly_reports,
        total: avg_total_reports,
    };

    Ok(JsonServerStats {
        timestamp: now,
        users: users_cohort,
        reports: reports_cohort,
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
