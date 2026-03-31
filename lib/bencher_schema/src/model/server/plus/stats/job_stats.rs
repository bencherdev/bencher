use bencher_json::system::server::{
    JsonCohort, JsonCohortAvg, JsonTopJobCohort, JsonTopJobProject, JsonTopJobProjects,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::HttpError;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::project::QueryProject, schema,
};

use super::{ProjectState, TOP_PROJECTS, median};

pub(super) struct JobStats {
    pub seconds: JsonCohort,
    pub seconds_per_report: JsonCohortAvg,
    pub top_projects: JsonTopJobCohort,
}

impl JobStats {
    #[expect(clippy::cast_sign_loss, reason = "duration is always positive")]
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let mut weekly_durations = get_durations_by_report(conn, Some(this_week), state)?;
        let weekly_total: i64 = weekly_durations.iter().sum();
        let weekly_median = median(&mut weekly_durations);
        let weekly_top = get_top_job_projects(conn, Some(this_week), state)?;

        let mut monthly_durations = get_durations_by_report(conn, Some(this_month), state)?;
        let monthly_total: i64 = monthly_durations.iter().sum();
        let monthly_median = median(&mut monthly_durations);
        let monthly_top = get_top_job_projects(conn, Some(this_month), state)?;

        let mut total_durations = get_durations_by_report(conn, None, state)?;
        let total_total: i64 = total_durations.iter().sum();
        let total_median = median(&mut total_durations);
        let total_top = get_top_job_projects(conn, None, state)?;

        let seconds = JsonCohort {
            week: weekly_total as u64,
            month: monthly_total as u64,
            total: total_total as u64,
        };

        let seconds_per_report = JsonCohortAvg {
            week: weekly_median,
            month: monthly_median,
            total: total_median,
        };

        let top_projects = JsonTopJobCohort {
            week: top_job_projects(weekly_top, weekly_total),
            month: top_job_projects(monthly_top, monthly_total),
            total: top_job_projects(total_top, total_total),
        };

        Ok(Self {
            seconds,
            seconds_per_report,
            top_projects,
        })
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_durations_by_report(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<Vec<i64>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::job_duration_by_report::table
                .inner_join(schema::report::table)
                .select(schema::job_duration_by_report::job_duration)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Job))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table
                        .inner_join(schema::project::table.inner_join(schema::organization::table)),
                )
                .select(schema::job_duration_by_report::job_duration)
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
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Job))
        },
        ProjectState::Plus => {
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table.inner_join(
                        schema::project::table.inner_join(
                            schema::organization::table
                                .inner_join(schema::plan::table.on(
                                    schema::plan::organization_id.eq(schema::organization::id),
                                )),
                        ),
                    ),
                )
                .filter(
                    schema::plan::metered_plan
                        .is_not_null()
                        .or(schema::plan::licensed_plan.is_not_null()),
                )
                .select(schema::job_duration_by_report::job_duration)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Job))
        },
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_top_job_projects(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<Vec<(QueryProject, i64)>, HttpError> {
    match state {
        ProjectState::All => {
            #[expect(clippy::cast_possible_wrap, reason = "const")]
            let mut query = schema::job_duration_by_report::table
                .inner_join(schema::report::table.inner_join(schema::project::table))
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::job_duration_by_report::job_duration),
                ))
                .order(diesel::dsl::sum(schema::job_duration_by_report::job_duration).desc())
                .limit(TOP_PROJECTS as i64)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<(QueryProject, Option<i64>)>(conn)
                .map(|v| {
                    v.into_iter()
                        .map(|(project, sum)| (project, sum.unwrap_or(0)))
                        .collect()
                })
                .map_err(resource_not_found_err!(Project))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            #[expect(clippy::cast_possible_wrap, reason = "const")]
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table
                        .inner_join(schema::project::table.inner_join(schema::organization::table)),
                )
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::job_duration_by_report::job_duration),
                ))
                .order(diesel::dsl::sum(schema::job_duration_by_report::job_duration).desc())
                .limit(TOP_PROJECTS as i64)
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
                .load::<(QueryProject, Option<i64>)>(conn)
                .map(|v| {
                    v.into_iter()
                        .map(|(project, sum)| (project, sum.unwrap_or(0)))
                        .collect()
                })
                .map_err(resource_not_found_err!(Project))
        },
        ProjectState::Plus => {
            #[expect(clippy::cast_possible_wrap, reason = "const")]
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table.inner_join(
                        schema::project::table.inner_join(
                            schema::organization::table
                                .inner_join(schema::plan::table.on(
                                    schema::plan::organization_id.eq(schema::organization::id),
                                )),
                        ),
                    ),
                )
                .filter(
                    schema::plan::metered_plan
                        .is_not_null()
                        .or(schema::plan::licensed_plan.is_not_null()),
                )
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::job_duration_by_report::job_duration),
                ))
                .order(diesel::dsl::sum(schema::job_duration_by_report::job_duration).desc())
                .limit(TOP_PROJECTS as i64)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<(QueryProject, Option<i64>)>(conn)
                .map(|v| {
                    v.into_iter()
                        .map(|(project, sum)| (project, sum.unwrap_or(0)))
                        .collect()
                })
                .map_err(resource_not_found_err!(Project))
        },
    }
}

#[expect(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn top_job_projects(project_durations: Vec<(QueryProject, i64)>, total: i64) -> JsonTopJobProjects {
    project_durations
        .into_iter()
        .map(|(project, duration)| JsonTopJobProject {
            name: project.name,
            uuid: project.uuid,
            seconds: duration as u64,
            percentage: if total > 0 {
                duration as f64 / total as f64
            } else {
                0.0
            },
        })
        .collect()
}
