use bencher_json::{JsonUsers, system::server::JsonCohort};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, model::user::QueryUser, schema};

pub(super) struct UsersStats {
    pub admins: Option<JsonUsers>,
    pub users: JsonCohort,
}

impl UsersStats {
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        is_bencher_cloud: bool,
    ) -> Result<Self, HttpError> {
        let admins = if is_bencher_cloud {
            None
        } else {
            Some(get_admins(conn)?)
        };
        let users = get_users(conn, this_week, this_month)?;

        Ok(Self { admins, users })
    }
}

fn get_admins(conn: &mut DbConnection) -> Result<JsonUsers, HttpError> {
    Ok(schema::user::table
        .filter(schema::user::admin.eq(true))
        .load::<QueryUser>(conn)
        .map_err(resource_not_found_err!(User))?
        .into_iter()
        .map(QueryUser::into_json)
        .collect())
}

#[expect(clippy::cast_sign_loss, reason = "count is always positive")]
fn get_users(
    conn: &mut DbConnection,
    this_week: i64,
    this_month: i64,
) -> Result<JsonCohort, HttpError> {
    let weekly_users = schema::user::table
        .filter(schema::user::created.ge(this_week))
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let monthly_users = schema::user::table
        .filter(schema::user::created.ge(this_month))
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    let total_users = schema::user::table
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(User))?;

    Ok(JsonCohort {
        week: weekly_users as u64,
        month: monthly_users as u64,
        total: total_users as u64,
    })
}
