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

            let (start_time, end_time) = context.rate_limiting.window();
            let window_usage: u32 = $crate::schema::$table::table
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

            context.rate_limiting.check_claimable_limit(
                is_claimed,
                window_usage,
                |rate_limit| $crate::context::RateLimitingError::UnclaimedProject {
                    project: query_project.clone(),
                    resource: $crate::error::BencherResource::$resource,
                    rate_limit,
                },
                |rate_limit| $crate::context::RateLimitingError::ClaimedProject {
                    project: query_project.clone(),
                    resource: $crate::error::BencherResource::$resource,
                    rate_limit,
                },
            )
        }
    };
}

pub(crate) use fn_rate_limit;
