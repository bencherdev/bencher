use bencher_json::system::server::JsonCohort;
use diesel::{
    AggregateExpressionMethods as _, BoolExpressionMethods as _, ExpressionMethods as _,
    JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, schema};

use super::ProjectState;

pub(super) struct ProjectsStats {
    pub projects: JsonCohort,
}

impl ProjectsStats {
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let projects = get_projects_cohort(conn, this_week, this_month, state)?;

        Ok(Self { projects })
    }
}

#[expect(clippy::cast_sign_loss, reason = "count is always positive")]
fn get_projects_cohort(
    conn: &mut DbConnection,
    this_week: i64,
    this_month: i64,
    state: ProjectState,
) -> Result<JsonCohort, HttpError> {
    let weekly_projects = get_project_count(conn, Some(this_week), state)?;
    let monthly_projects = get_project_count(conn, Some(this_month), state)?;
    let total_projects = get_project_count(conn, None, state)?;

    Ok(JsonCohort {
        week: weekly_projects as u64,
        month: monthly_projects as u64,
        total: total_projects as u64,
    })
}

fn get_project_count(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<i64, HttpError> {
    match state {
        // Intentionally includes soft-deleted projects for server admin stats
        ProjectState::All => {
            let mut query = schema::project::table
                .select(diesel::dsl::count(schema::project::id).aggregate_distinct())
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .get_result::<i64>(conn)
                .map_err(resource_not_found_err!(Project))
        },
        // Intentionally includes soft-deleted projects for server admin stats
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::project::table
                .inner_join(schema::organization::table)
                .select(diesel::dsl::count(schema::project::id))
                .into_boxed();

            let is_claimed = matches!(state, ProjectState::Claimed);
            let org_has_roles = schema::organization_role::table
                .filter(schema::organization_role::organization_id.eq(schema::organization::id));
            query = if is_claimed {
                query.filter(diesel::dsl::exists(org_has_roles))
            } else {
                query.filter(diesel::dsl::not(diesel::dsl::exists(org_has_roles)))
            };

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .first::<i64>(conn)
                .map_err(resource_not_found_err!(Project))
        },
        // Intentionally includes soft-deleted projects for server admin stats
        ProjectState::Plus => {
            let mut query = schema::project::table
                .inner_join(
                    schema::organization::table.inner_join(
                        schema::plan::table
                            .on(schema::plan::organization_id.eq(schema::organization::id)),
                    ),
                )
                .filter(
                    schema::plan::metered_plan
                        .is_not_null()
                        .or(schema::plan::licensed_plan.is_not_null()),
                )
                .select(diesel::dsl::count(schema::project::id))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::project::created.ge(since));
            }

            query
                .first::<i64>(conn)
                .map_err(resource_not_found_err!(Project))
        },
    }
}
