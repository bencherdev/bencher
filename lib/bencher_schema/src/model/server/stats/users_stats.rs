use bencher_json::{JsonUsers, system::server::JsonCohort};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::user::QueryUser, schema,
    yield_connection_lock,
};

pub(super) struct UsersStats {
    pub admins: Option<JsonUsers>,
    pub users: JsonCohort,
}

impl UsersStats {
    pub async fn new(
        db_connection: &Mutex<DbConnection>,
        this_week: i64,
        this_month: i64,
        is_bencher_cloud: bool,
    ) -> Result<Self, HttpError> {
        let admins = get_admins(db_connection, is_bencher_cloud).await?;
        let users = get_users(db_connection, this_week, this_month).await?;

        Ok(Self { admins, users })
    }
}

async fn get_admins(
    db_connection: &Mutex<DbConnection>,
    is_bencher_cloud: bool,
) -> Result<Option<JsonUsers>, HttpError> {
    Ok(if is_bencher_cloud {
        None
    } else {
        Some(yield_connection_lock!(db_connection, |conn| {
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

#[expect(clippy::cast_sign_loss, reason = "count is always positive")]
async fn get_users(
    db_connection: &Mutex<DbConnection>,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_users = yield_connection_lock!(db_connection, |conn| schema::user::table
        .filter(schema::user::created.ge(this_week))
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?);

    let monthly_users = yield_connection_lock!(db_connection, |conn| schema::user::table
        .filter(schema::user::created.ge(this_month))
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?);

    let total_users = yield_connection_lock!(db_connection, |conn| schema::user::table
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?);

    Ok(JsonCohort {
        week: weekly_users as u64,
        month: monthly_users as u64,
        total: total_users as u64,
    })
}
