use bencher_json::system::server::JsonCohort;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{connection_lock, context::DbConnection, error::resource_not_found_err, schema};

use super::ProjectState;

pub(super) struct ProjectsStats {
    pub projects: JsonCohort,
}

impl ProjectsStats {
    pub async fn new(
        db_connection: &Mutex<DbConnection>,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let projects = get_projects_cohort(db_connection, this_week, this_month, state).await?;

        Ok(Self { projects })
    }
}

#[expect(clippy::cast_sign_loss, reason = "count is always positive")]
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
                .first::<i64>(connection_lock!(db_connection))
                .map_err(resource_not_found_err!(Project))
        },
    }
}
