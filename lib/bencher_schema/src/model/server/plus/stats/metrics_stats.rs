use bencher_json::system::server::{
    JsonCohort, JsonCohortAvg, JsonTopCohort, JsonTopProject, JsonTopProjects,
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

pub(super) struct MetricsStats {
    pub metrics: JsonCohort,
    pub metrics_per_report: JsonCohortAvg,
    pub top_projects: JsonTopCohort,
}

impl MetricsStats {
    #[expect(clippy::cast_sign_loss, reason = "count is always positive")]
    pub fn new(
        conn: &mut DbConnection,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let mut weekly_metrics = get_metrics_by_report(conn, Some(this_week), state)?;
        let weekly_metrics_total: i64 = weekly_metrics.iter().sum();
        let weekly_metrics_per_project = median(&mut weekly_metrics);
        let weekly_top_projects = get_top_projects(conn, Some(this_week), state)?;

        let mut monthly_metrics = get_metrics_by_report(conn, Some(this_month), state)?;
        let monthly_metrics_total: i64 = monthly_metrics.iter().sum();
        let monthly_metrics_per_project = median(&mut monthly_metrics);
        let monthly_top_projects = get_top_projects(conn, Some(this_month), state)?;

        let mut total_metrics = get_metrics_by_report(conn, None, state)?;
        let total_metrics_total: i64 = total_metrics.iter().sum();
        let total_metrics_per_project = median(&mut total_metrics);
        let total_top_projects = get_top_projects(conn, None, state)?;

        let metrics = JsonCohort {
            week: weekly_metrics_total as u64,
            month: monthly_metrics_total as u64,
            total: total_metrics_total as u64,
        };

        let metrics_per_report = JsonCohortAvg {
            week: weekly_metrics_per_project,
            month: monthly_metrics_per_project,
            total: total_metrics_per_project,
        };

        let top_projects = JsonTopCohort {
            week: top_projects(weekly_top_projects, weekly_metrics_total),
            month: top_projects(monthly_top_projects, monthly_metrics_total),
            total: top_projects(total_top_projects, total_metrics_total),
        };

        Ok(Self {
            metrics,
            metrics_per_report,
            top_projects,
        })
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_metrics_by_report(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<Vec<i64>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::metric_count_by_report::table
                .inner_join(schema::report::table)
                .select(schema::metric_count_by_report::metric_count)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Metric))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::metric_count_by_report::table
                .inner_join(
                    schema::report::table
                        .inner_join(schema::project::table.inner_join(schema::organization::table)),
                )
                .select(schema::metric_count_by_report::metric_count)
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
                .map_err(resource_not_found_err!(Metric))
        },
        ProjectState::Plus => {
            let mut query = schema::metric_count_by_report::table
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
                .select(schema::metric_count_by_report::metric_count)
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            query
                .load::<i32>(conn)
                .map(|v| v.into_iter().map(i64::from).collect())
                .map_err(resource_not_found_err!(Metric))
        },
    }
}

// Intentionally includes soft-deleted projects for server admin stats
fn get_top_projects(
    conn: &mut DbConnection,
    since: Option<i64>,
    state: ProjectState,
) -> Result<Vec<(QueryProject, i64)>, HttpError> {
    match state {
        ProjectState::All => {
            #[expect(clippy::cast_possible_wrap, reason = "const")]
            let mut query = schema::metric_count_by_report::table
                .inner_join(schema::report::table.inner_join(schema::project::table))
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::metric_count_by_report::metric_count),
                ))
                .order(diesel::dsl::sum(schema::metric_count_by_report::metric_count).desc())
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
            let mut query = schema::metric_count_by_report::table
                .inner_join(
                    schema::report::table
                        .inner_join(schema::project::table.inner_join(schema::organization::table)),
                )
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::sum(schema::metric_count_by_report::metric_count),
                ))
                .order(diesel::dsl::sum(schema::metric_count_by_report::metric_count).desc())
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
            let mut query = schema::metric_count_by_report::table
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
                    diesel::dsl::sum(schema::metric_count_by_report::metric_count),
                ))
                .order(diesel::dsl::sum(schema::metric_count_by_report::metric_count).desc())
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
fn top_projects(project_metrics: Vec<(QueryProject, i64)>, total: i64) -> JsonTopProjects {
    project_metrics
        .into_iter()
        .map(|(project, metrics)| JsonTopProject {
            name: project.name,
            uuid: project.uuid,
            metrics: metrics as u64,
            percentage: if total > 0 {
                metrics as f64 / total as f64
            } else {
                0.0
            },
        })
        .collect()
}
