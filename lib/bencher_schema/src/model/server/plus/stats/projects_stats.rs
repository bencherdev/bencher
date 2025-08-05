use bencher_json::system::server::JsonCohort;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, schema};

use super::ProjectState;

pub(super) struct ProjectsStats {
    pub projects: JsonCohort,
}

impl ProjectsStats {
    pub fn new(
        db_connection: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let projects = get_projects_cohort(db_connection, this_week, this_month, state)?;

        Ok(Self { projects })
    }
}

#[expect(clippy::cast_sign_loss, reason = "count is always positive")]
fn get_projects_cohort(
    db_connection: &mut DbConnection,
    this_week: i64,
    this_month: i64,
    state: ProjectState,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = get_project_count(db_connection, Some(this_week), state)?;
    let monthly_projects = get_project_count(db_connection, Some(this_month), state)?;
    let total_projects = get_project_count(db_connection, None, state)?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    })
}

fn get_project_count(
    db_connection: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<i64, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::project::table
                .select(diesel::dsl::count_distinct(schema::project::id))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .get_result::<i64>(db_connection)
                .map_err(resource_not_found_err!(Project))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::project::table
                .inner_join(schema::organization::table.left_join(schema::organization_role::table))
                .select(diesel::dsl::count_distinct(schema::project::id))
                .into_boxed();

            query = match state {
                #[expect(
                    clippy::unreachable,
                    reason = "match above ensures this is unreachable"
                )]
                ProjectState::All => unreachable!(),
                ProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                ProjectState::Claimed => query.filter(schema::organization_role::id.is_not_null()),
            };

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .first::<i64>(db_connection)
                .map_err(resource_not_found_err!(Project))
        },
    }
}
