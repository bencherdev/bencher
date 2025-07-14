use bencher_json::system::server::{
    JsonCohort, JsonCohortAvg, JsonTopCohort, JsonTopProject, JsonTopProjects,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::project::QueryProject, schema,
    yield_connection_lock,
};

use super::{ProjectState, TOP_PROJECTS, median};

pub(super) struct MetricsStats {
    pub metrics: JsonCohort,
    pub metrics_per_report: JsonCohortAvg,
    pub top_projects: JsonTopCohort,
}

impl MetricsStats {
    #[expect(clippy::cast_sign_loss, reason = "count is always positive")]
    pub async fn new(
        db_connection: &Mutex<DbConnection>,
        this_week: i64,
        this_month: i64,
        state: ProjectState,
    ) -> Result<Self, HttpError> {
        let mut weekly_metrics =
            get_metrics_by_report(db_connection, Some(this_week), &state).await?;
        let weekly_metrics_total: i64 = weekly_metrics.iter().sum();
        let weekly_metrics_per_project = median(&mut weekly_metrics);
        let weekly_top_projects = get_top_projects(db_connection, Some(this_week), &state).await?;

        let mut monthly_metrics =
            get_metrics_by_report(db_connection, Some(this_month), &state).await?;
        let monthly_metrics_total: i64 = monthly_metrics.iter().sum();
        let monthly_metrics_per_project = median(&mut monthly_metrics);
        let monthly_top_projects =
            get_top_projects(db_connection, Some(this_month), &state).await?;

        let mut total_metrics = get_metrics_by_report(db_connection, None, &state).await?;
        let total_metrics_total: i64 = total_metrics.iter().sum();
        let total_metrics_per_project = median(&mut total_metrics);
        let total_top_projects = get_top_projects(db_connection, None, &state).await?;

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

async fn get_metrics_by_report(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<Vec<i64>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(schema::report::table))
                .group_by(schema::report::id)
                .select(diesel::dsl::count_distinct(schema::metric::id))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            yield_connection_lock!(db_connection, |conn| query
                .load::<i64>(conn)
                .map_err(resource_not_found_err!(Metric)))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                ))
                .group_by(schema::report::id)
                .select(diesel::dsl::count_distinct(schema::metric::id))
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
                query = query.filter(schema::report::created.ge(since));
            }

            yield_connection_lock!(db_connection, |conn| query
                .load::<i64>(conn)
                .map_err(resource_not_found_err!(Metric)))
        },
    }
}

async fn get_top_projects(
    db_connection: &Mutex<DbConnection>,
    since: Option<i64>,
    state: &ProjectState,
) -> Result<Vec<(QueryProject, i64)>, HttpError> {
    match state {
        ProjectState::All => {
            let mut query = schema::metric::table
                .inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::report::table.inner_join(schema::project::table)),
                )
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::count_distinct(schema::metric::id),
                ))
                .into_boxed();

            if let Some(since) = since {
                query = query.filter(schema::report::created.ge(since));
            }

            yield_connection_lock!(db_connection, |conn| query
                .load::<(QueryProject, i64)>(conn)
                .map_err(resource_not_found_err!(Project)))
        },
        ProjectState::Unclaimed | ProjectState::Claimed => {
            let mut query = schema::metric::table
                .inner_join(schema::report_benchmark::table.inner_join(
                    schema::report::table.inner_join(schema::project::table.inner_join(
                        schema::organization::table.left_join(schema::organization_role::table),
                    )),
                ))
                .group_by(schema::project::id)
                .select((
                    QueryProject::as_select(),
                    diesel::dsl::count_distinct(schema::metric::id),
                ))
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
                query = query.filter(schema::report::created.ge(since));
            }

            yield_connection_lock!(db_connection, |conn| query
                .load::<(QueryProject, i64)>(conn)
                .map_err(resource_not_found_err!(Project)))
        },
    }
}

#[expect(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn top_projects(mut project_metrics: Vec<(QueryProject, i64)>, total: i64) -> JsonTopProjects {
    project_metrics.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    project_metrics.reverse();
    if project_metrics.len() > TOP_PROJECTS {
        project_metrics.truncate(TOP_PROJECTS);
    }
    project_metrics
        .into_iter()
        .map(|(project, metrics)| JsonTopProject {
            name: project.name,
            uuid: project.uuid,
            metrics: metrics as u64,
            percentage: metrics as f64 / total as f64,
        })
        .collect()
}
