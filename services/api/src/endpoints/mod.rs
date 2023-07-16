use dropshot::ApiDescription;

pub mod endpoint;
pub mod method;
pub mod organization;
pub mod project;
pub mod resource;
pub mod system;
pub mod user;

pub use endpoint::Endpoint;
pub use method::Method;
pub use resource::Resource;

use crate::{context::ApiContext, ApiError};

pub struct Api;

impl Api {
    pub fn register(
        api: &mut ApiDescription<ApiContext>,
        http_options: bool,
    ) -> Result<(), ApiError> {
        Self::register_inner(api, http_options).map_err(ApiError::Register)
    }

    fn register_inner(
        api: &mut ApiDescription<ApiContext>,
        http_options: bool,
    ) -> Result<(), String> {
        // Auth
        if http_options {
            api.register(system::auth::signup::auth_signup_options)?;
            api.register(system::auth::login::auth_login_options)?;
            api.register(system::auth::confirm::auth_confirm_options)?;
        }
        api.register(system::auth::signup::auth_signup_post)?;
        api.register(system::auth::login::auth_login_post)?;
        api.register(system::auth::confirm::auth_confirm_post)?;

        // Organizations
        if http_options {
            api.register(organization::organizations::organizations_options)?;
            api.register(organization::organizations::organization_options)?;
        }
        api.register(organization::organizations::organizations_get)?;
        api.register(organization::organizations::organization_post)?;
        api.register(organization::organizations::organization_get)?;
        api.register(organization::organizations::organization_patch)?;

        // Organization Permission
        if http_options {
            api.register(organization::allowed::org_allowed_options)?;
        }
        api.register(organization::allowed::org_allowed_get)?;

        // Organization Members
        if http_options {
            api.register(organization::members::org_members_options)?;
            api.register(organization::members::org_member_options)?;
        }
        api.register(organization::members::org_members_get)?;
        api.register(organization::members::org_member_post)?;
        api.register(organization::members::org_member_get)?;
        api.register(organization::members::org_member_patch)?;
        api.register(organization::members::org_member_delete)?;

        // Organization Projects
        if http_options {
            api.register(organization::projects::org_projects_options)?;
            api.register(organization::projects::org_project_options)?;
        }
        api.register(organization::projects::org_projects_get)?;
        api.register(organization::projects::org_project_post)?;
        api.register(organization::projects::org_project_get)?;
        api.register(organization::projects::org_project_patch)?;
        api.register(organization::projects::org_project_delete)?;

        #[cfg(feature = "plus")]
        {
            // Organization Metered Subscription Plan
            if http_options {
                api.register(organization::plan::org_plan_options)?;
            }
            api.register(organization::plan::org_plan_post)?;
            api.register(organization::plan::org_plan_get)?;

            // Organization Usage
            if http_options {
                api.register(organization::usage::org_usage_options)?;
            }
            api.register(organization::usage::org_usage_get)?;
        }

        // Projects
        // All of a projects's GET APIs are public if the project is public
        if http_options {
            api.register(project::projects::projects_options)?;
            api.register(project::projects::project_options)?;
        }
        api.register(project::projects::projects_get)?;
        api.register(project::projects::project_get)?;
        api.register(project::projects::project_patch)?;
        api.register(project::projects::project_delete)?;

        // Perf
        if http_options {
            api.register(project::perf::proj_perf_options)?;
        }
        api.register(project::perf::proj_perf_get)?;

        // Perf Image
        if http_options {
            api.register(project::perf::img::proj_perf_img_options)?;
        }
        api.register(project::perf::img::proj_perf_img_get)?;

        // Reports
        if http_options {
            api.register(project::reports::proj_reports_options)?;
            api.register(project::reports::proj_report_options)?;
        }
        api.register(project::reports::proj_report_post)?;
        api.register(project::reports::proj_reports_get)?;
        api.register(project::reports::proj_report_get)?;
        api.register(project::reports::proj_report_delete)?;

        // Metric Kinds
        if http_options {
            api.register(project::metric_kinds::proj_metric_kinds_options)?;
            api.register(project::metric_kinds::proj_metric_kind_options)?;
        }
        api.register(project::metric_kinds::proj_metric_kinds_get)?;
        api.register(project::metric_kinds::proj_metric_kind_post)?;
        api.register(project::metric_kinds::proj_metric_kind_get)?;
        api.register(project::metric_kinds::proj_metric_kind_patch)?;

        // Branches
        if http_options {
            api.register(project::branches::proj_branches_options)?;
            api.register(project::branches::proj_branch_options)?;
        }
        api.register(project::branches::proj_branches_get)?;
        api.register(project::branches::proj_branch_post)?;
        api.register(project::branches::proj_branch_get)?;
        api.register(project::branches::proj_branch_patch)?;

        // Testbeds
        if http_options {
            api.register(project::testbeds::proj_testbeds_options)?;
            api.register(project::testbeds::proj_testbed_options)?;
        }
        api.register(project::testbeds::proj_testbeds_get)?;
        api.register(project::testbeds::proj_testbed_post)?;
        api.register(project::testbeds::proj_testbed_get)?;
        api.register(project::testbeds::proj_testbed_patch)?;

        // Benchmarks
        if http_options {
            api.register(project::benchmarks::proj_benchmarks_options)?;
            api.register(project::benchmarks::proj_benchmark_options)?;
        }
        api.register(project::benchmarks::proj_benchmarks_get)?;
        api.register(project::benchmarks::proj_benchmark_post)?;
        api.register(project::benchmarks::proj_benchmark_get)?;
        api.register(project::benchmarks::proj_benchmark_patch)?;

        // Thresholds
        if http_options {
            api.register(project::thresholds::proj_thresholds_options)?;
            api.register(project::thresholds::proj_threshold_options)?;
        }
        api.register(project::thresholds::proj_thresholds_get)?;
        api.register(project::thresholds::proj_threshold_post)?;
        api.register(project::thresholds::proj_threshold_get)?;
        api.register(project::thresholds::proj_threshold_put)?;

        // Threshold Statistics
        if http_options {
            api.register(project::thresholds::statistics::proj_statistic_options)?;
        }
        api.register(project::thresholds::statistics::proj_statistic_get)?;

        // Threshold Alerts
        if http_options {
            api.register(project::thresholds::alerts::proj_alerts_options)?;
            api.register(project::thresholds::alerts::proj_alert_options)?;
        }
        api.register(project::thresholds::alerts::proj_alerts_get)?;
        api.register(project::thresholds::alerts::proj_alert_get)?;

        // Users
        if http_options {
            api.register(user::users::user_options)?;
        }
        api.register(user::users::user_get)?;

        // Tokens
        if http_options {
            api.register(user::tokens::user_tokens_options)?;
            api.register(user::tokens::user_token_options)?;
        }
        api.register(user::tokens::user_tokens_get)?;
        api.register(user::tokens::user_token_post)?;
        api.register(user::tokens::user_token_get)?;

        // Server
        if http_options {
            api.register(system::server::ping::server_ping_options)?;
            api.register(system::server::version::server_version_options)?;
            api.register(system::server::restart::server_restart_options)?;
            api.register(system::server::config::server_config_options)?;
            api.register(system::server::config::endpoint::server_config_endpoint_options)?;
            api.register(system::server::backup::server_backup_options)?;
        }
        api.register(system::server::ping::server_ping_get)?;
        api.register(system::server::version::server_version_get)?;
        api.register(system::server::restart::server_restart_post)?;
        api.register(system::server::config::server_config_get)?;
        api.register(system::server::config::server_config_put)?;
        api.register(system::server::config::endpoint::server_config_endpoint_get)?;
        api.register(system::server::backup::server_backup_post)?;

        Ok(())
    }
}
