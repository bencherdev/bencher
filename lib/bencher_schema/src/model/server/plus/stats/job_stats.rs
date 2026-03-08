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

use super::{TOP_PROJECTS, median};

/// Which subset of projects to include in job stats.
#[derive(Debug, Clone, Copy)]
pub(super) enum JobProjectState {
    All,
    Unclaimed,
    Claimed,
    Plus,
}

pub(super) struct JobStats {
    pub job_duration: JsonCohort,
    pub job_duration_per_report: JsonCohortAvg,
    pub top_job_projects: JsonTopJobCohort,
}

impl JobStats {
    #[expect(clippy::cast_sign_loss, reason = "duration is always positive")]
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: JobProjectState,
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

        let job_duration = JsonCohort {
            week: weekly_total as u64,
            month: monthly_total as u64,
            total: total_total as u64,
        };

        let job_duration_per_report = JsonCohortAvg {
            week: weekly_median,
            month: monthly_median,
            total: total_median,
        };

        let top_job_projects = JsonTopJobCohort {
            week: top_job_projects(weekly_top, weekly_total),
            month: top_job_projects(monthly_top, monthly_total),
            total: top_job_projects(total_top, total_total),
        };

        Ok(Self {
            job_duration,
            job_duration_per_report,
            top_job_projects,
        })
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_durations_by_report(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: JobProjectState,
) -> Result<Vec<i64>, HttpError> {
    match state {
        JobProjectState::All => {
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
        JobProjectState::Unclaimed | JobProjectState::Claimed => {
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                )
                .select(schema::job_duration_by_report::job_duration)
                .into_boxed();

            query = match state {
                #[expect(
                    clippy::unreachable,
                    reason = "match above ensures this is unreachable"
                )]
                JobProjectState::All | JobProjectState::Plus => unreachable!(),
                JobProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                JobProjectState::Claimed => {
                    query.filter(schema::organization_role::id.is_not_null())
                },
            };

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Job))
        },
        JobProjectState::Plus => {
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
#[expect(clippy::too_many_lines, reason = "4-variant match with Diesel queries")]
fn get_top_job_projects(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: JobProjectState,
) -> Result<Vec<(QueryProject, i64)>, HttpError> {
    match state {
        JobProjectState::All => {
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
        JobProjectState::Unclaimed | JobProjectState::Claimed => {
            #[expect(clippy::cast_possible_wrap, reason = "const")]
            let mut query = schema::job_duration_by_report::table
                .inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                )
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::job_duration_by_report::job_duration),
                ))
                .order(diesel::dsl::sum(schema::job_duration_by_report::job_duration).desc())
                .limit(TOP_PROJECTS as i64)
                .into_boxed();

            query = match state {
                #[expect(
                    clippy::unreachable,
                    reason = "match above ensures this is unreachable"
                )]
                JobProjectState::All | JobProjectState::Plus => unreachable!(),
                JobProjectState::Unclaimed => query.filter(schema::organization_role::id.is_null()),
                JobProjectState::Claimed => {
                    query.filter(schema::organization_role::id.is_not_null())
                },
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
        JobProjectState::Plus => {
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
            duration: duration as u64,
            percentage: if total > 0 {
                duration as f64 / total as f64
            } else {
                0.0
            },
        })
        .collect()
}
