use std::time::Duration;

use bencher_json::DateTime;

use crate::{
    error::BencherResource,
    model::{organization::QueryOrganization, project::QueryProject, user::QueryUser},
};

pub const DAY: Duration = Duration::from_secs(24 * 60 * 60);
pub const UNCLAIMED_RATE_LIMIT: u32 = u8::MAX as u32;
pub const CLAIMED_RATE_LIMIT: u32 = u16::MAX as u32;

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Organization ({uuid}) has exceeded the daily rate limit ({UNCLAIMED_RATE_LIMIT}) for {resource} creation. Please, reduce your daily usage.", uuid = organization.uuid)]
    Organization {
        organization: QueryOrganization,
        resource: BencherResource,
    },
    #[error("Unclaimed project ({uuid}) has exceeded the daily rate limit ({UNCLAIMED_RATE_LIMIT}) for {resource} creation. Please, reduce your daily usage or claim the project: https://bencher.dev/auth/signup?claim={uuid}", uuid = project.uuid)]
    UnclaimedProject {
        project: QueryProject,
        resource: BencherResource,
    },
    #[error("Claimed project ({uuid}) has exceeded the daily rate limit ({CLAIMED_RATE_LIMIT}) for {resource} creation. Please, reduce your daily usage.", uuid = project.uuid)]
    ClaimedProject {
        project: QueryProject,
        resource: BencherResource,
    },
    #[error("User ({uuid}) has exceeded the daily rate limit ({UNCLAIMED_RATE_LIMIT}) for {resource} creation. Please, reduce your daily usage.", uuid = user.uuid)]
    User {
        user: QueryUser,
        resource: BencherResource,
    },
}

pub fn one_day() -> (DateTime, DateTime) {
    let end_time = chrono::Utc::now();
    let start_time = end_time - DAY;
    (start_time.into(), end_time.into())
}

#[macro_export]
macro_rules! fn_rate_limit {
    ($table:ident, $resource:ident) => {
        pub async fn rate_limit(
            context: &ApiContext,
            project_id: ProjectId,
        ) -> Result<(), HttpError> {
            let query_project = QueryProject::get(conn_lock!(context), project_id)?;
            let query_organization = query_project.organization(conn_lock!(context))?;
            let is_claimed = query_organization.is_claimed(conn_lock!(context))?;

            let (start_time, end_time) = $crate::macros::rate_limit::one_day();
            let creation_count: u32 = $crate::schema::$table::table
                .filter($crate::schema::$table::project_id.eq(project_id))
                .filter($crate::schema::$table::created.ge(start_time))
                .filter($crate::schema::$table::created.le(end_time))
                .count()
                .get_result::<i64>(conn_lock!(context))
                .map_err($crate::error::resource_not_found_err!($resource, (project_id, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    $crate::error::issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for project ({project_id}) between {start_time} and {end_time}.", resource = $crate::error::BencherResource::$resource),
                    e
                    )}
                )?;

            match (is_claimed, creation_count) {
                (false, creation_count)
                    if creation_count >= $crate::macros::rate_limit::UNCLAIMED_RATE_LIMIT =>
                {
                    Err($crate::error::too_many_requests(
                        $crate::macros::rate_limit::RateLimitError::UnclaimedProject {
                            project: query_project,
                            resource: $crate::error::BencherResource::$resource,
                        },
                    ))
                },
                (true, creation_count)
                    if creation_count >= $crate::macros::rate_limit::CLAIMED_RATE_LIMIT =>
                {
                    Err($crate::error::too_many_requests(
                        $crate::macros::rate_limit::RateLimitError::ClaimedProject {
                            project: query_project,
                            resource: $crate::error::BencherResource::$resource,
                        },
                    ))
                },
                (_, _) => Ok(()),
            }
        }
    };
}

pub(crate) use fn_rate_limit;
