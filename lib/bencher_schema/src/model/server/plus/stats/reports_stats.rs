use bencher_json::system::server::{JsonCohort, JsonCohortAvg};
use diesel::{
    AggregateExpressionMethods as _, BoolExpressionMethods as _, ExpressionMethods as _,
    JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, schema};

use super::{ProjectState, median};

pub(super) struct ReportsStats {
    pub active_projects: JsonCohort,
    pub reports: JsonCohort,
    pub reports_per_project: JsonCohortAvg,
}

impl ReportsStats {
    #[expect(clippy::cast_sign_loss, reason = "count is always positive")]
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let mut weekly_reports = get_reports_by_project(conn, Some(this_week), state)?;
        let weekly_active_projects = weekly_reports.len();
        let weekly_reports_total: i64 = weekly_reports.iter().sum();
        let weekly_reports_per_project = median(&mut weekly_reports);

        let mut monthly_reports = get_reports_by_project(conn, Some(this_month), state)?;
        let monthly_active_projects = monthly_reports.len();
        let monthly_reports_total: i64 = monthly_reports.iter().sum();
        let monthly_reports_per_project = median(&mut monthly_reports);

        let mut total_reports = get_reports_by_project(conn, None, state)?;
        let total_active_projects = total_reports.len();
        let total_reports_total: i64 = total_reports.iter().sum();
        let total_reports_per_project = median(&mut total_reports);

        let active_projects = JsonCohort {
            week: weekly_active_projects as u64,
            month: monthly_active_projects as u64,
            total: total_active_projects as u64,
        };

        let reports = JsonCohort {
            week: weekly_reports_total as u64,
            month: monthly_reports_total as u64,
            total: total_reports_total as u64,
        };

        let reports_per_project = JsonCohortAvg {
            week: weekly_reports_per_project,
            month: monthly_reports_per_project,
            total: total_reports_per_project,
        };

        Ok(Self {
            active_projects,
            reports,
            reports_per_project,
        })
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_reports_by_project(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<Vec<i64>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::report::table
                .group_by(schema::report::project_id)
                .select(diesel::dsl::count(schema::report::id).aggregate_distinct())
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i64>(conn)
                .map_err(resource_not_found_err!(Report))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::report::table
                .inner_join(schema::project::table.inner_join(schema::organization::table))
                .group_by(schema::report::project_id)
                .select(diesel::dsl::count(schema::report::id))
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
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i64>(conn)
                .map_err(resource_not_found_err!(Report))
        },
        ProjectState::Plus => {
            let mut query = schema::report::table
                .inner_join(
                    schema::project::table.inner_join(
                        schema::organization::table.inner_join(
                            schema::plan::table
                                .on(schema::plan::organization_id.eq(schema::organization::id)),
                        ),
                    ),
                )
                .filter(
                    schema::plan::metered_plan
                        .is_not_null()
                        .or(schema::plan::licensed_plan.is_not_null()),
                )
                .group_by(schema::report::project_id)
                .select(diesel::dsl::count(schema::report::id))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i64>(conn)
                .map_err(resource_not_found_err!(Report))
        },
    }
}
