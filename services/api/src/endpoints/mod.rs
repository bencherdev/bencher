#![allow(clippy::str_to_string)]

use dropshot::ApiDescription;

pub mod endpoint;
pub mod organization;
pub mod project;
pub mod system;
pub mod user;

pub use endpoint::Endpoint;

use crate::context::ApiContext;

pub struct Api;

impl Api {
    #[allow(clippy::too_many_lines)]
    pub fn register(
        api: &mut ApiDescription<ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), String> {
        // Root
        if http_options {
            api.register(system::root::server_root_options)?;
        }
        api.register(system::root::server_root_get)?;

        // Auth
        if http_options {
            api.register(system::auth::signup::auth_signup_options)?;
            api.register(system::auth::login::auth_login_options)?;
            api.register(system::auth::confirm::auth_confirm_options)?;
            api.register(system::auth::accept::auth_accept_options)?;
        }
        api.register(system::auth::signup::auth_signup_post)?;
        api.register(system::auth::login::auth_login_post)?;
        api.register(system::auth::confirm::auth_confirm_post)?;
        api.register(system::auth::accept::auth_accept_post)?;

        #[cfg(feature = "plus")]
        {
            // GitHub OAuth
            if http_options {
                api.register(system::auth::github::auth_github_options)?;
            }
            api.register(system::auth::github::auth_github_post)?;
        }

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
        }
        api.register(organization::projects::org_projects_get)?;
        api.register(organization::projects::org_project_post)?;

        #[cfg(feature = "plus")]
        {
            // Organization Plan
            // Bencher Cloud only
            if is_bencher_cloud {
                if http_options {
                    api.register(organization::plan::org_plan_options)?;
                }
                api.register(organization::plan::org_plan_get)?;
                api.register(organization::plan::org_plan_post)?;
                api.register(organization::plan::org_plan_delete)?;
            }

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

        // Project Permission
        if http_options {
            api.register(project::allowed::proj_allowed_options)?;
        }
        api.register(project::allowed::proj_allowed_get)?;

        // Reports
        if http_options {
            api.register(project::reports::proj_reports_options)?;
            api.register(project::reports::proj_report_options)?;
        }
        api.register(project::reports::proj_report_post)?;
        api.register(project::reports::proj_reports_get)?;
        api.register(project::reports::proj_report_get)?;
        api.register(project::reports::proj_report_delete)?;

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

        // Branches
        if http_options {
            api.register(project::branches::proj_branches_options)?;
            api.register(project::branches::proj_branch_options)?;
        }
        api.register(project::branches::proj_branches_get)?;
        api.register(project::branches::proj_branch_post)?;
        api.register(project::branches::proj_branch_get)?;
        api.register(project::branches::proj_branch_patch)?;
        api.register(project::branches::proj_branch_delete)?;

        // Testbeds
        if http_options {
            api.register(project::testbeds::proj_testbeds_options)?;
            api.register(project::testbeds::proj_testbed_options)?;
        }
        api.register(project::testbeds::proj_testbeds_get)?;
        api.register(project::testbeds::proj_testbed_post)?;
        api.register(project::testbeds::proj_testbed_get)?;
        api.register(project::testbeds::proj_testbed_patch)?;
        api.register(project::testbeds::proj_testbed_delete)?;

        // Benchmarks
        if http_options {
            api.register(project::benchmarks::proj_benchmarks_options)?;
            api.register(project::benchmarks::proj_benchmark_options)?;
        }
        api.register(project::benchmarks::proj_benchmarks_get)?;
        api.register(project::benchmarks::proj_benchmark_post)?;
        api.register(project::benchmarks::proj_benchmark_get)?;
        api.register(project::benchmarks::proj_benchmark_patch)?;
        api.register(project::benchmarks::proj_benchmark_delete)?;

        // Measures
        if http_options {
            api.register(project::measures::proj_measures_options)?;
            api.register(project::measures::proj_measure_options)?;
        }
        api.register(project::measures::proj_measures_get)?;
        api.register(project::measures::proj_measure_post)?;
        api.register(project::measures::proj_measure_get)?;
        api.register(project::measures::proj_measure_patch)?;
        api.register(project::measures::proj_measure_delete)?;

        // Thresholds
        if http_options {
            api.register(project::thresholds::proj_thresholds_options)?;
            api.register(project::thresholds::proj_threshold_options)?;
        }
        api.register(project::thresholds::proj_thresholds_get)?;
        api.register(project::thresholds::proj_threshold_post)?;
        api.register(project::thresholds::proj_threshold_get)?;
        api.register(project::thresholds::proj_threshold_put)?;
        api.register(project::thresholds::proj_threshold_delete)?;

        // Threshold Alerts
        if http_options {
            api.register(project::thresholds::alerts::proj_alerts_options)?;
            api.register(project::thresholds::alerts::proj_alert_options)?;
            api.register(project::thresholds::alerts::proj_alert_stats_options)?;
        }
        api.register(project::thresholds::alerts::proj_alerts_get)?;
        api.register(project::thresholds::alerts::proj_alert_get)?;
        api.register(project::thresholds::alerts::proj_alert_patch)?;
        api.register(project::thresholds::alerts::proj_alert_stats_get)?;

        // Users
        if http_options {
            api.register(user::users::users_options)?;
            api.register(user::users::user_options)?;
        }
        api.register(user::users::users_get)?;
        api.register(user::users::user_get)?;
        api.register(user::users::user_patch)?;

        // Tokens
        if http_options {
            api.register(user::tokens::user_tokens_options)?;
            api.register(user::tokens::user_token_options)?;
        }
        api.register(user::tokens::user_tokens_get)?;
        api.register(user::tokens::user_token_post)?;
        api.register(user::tokens::user_token_get)?;
        api.register(user::tokens::user_token_patch)?;

        // Server
        if http_options {
            api.register(system::server::version::server_version_options)?;
            api.register(system::server::spec::server_spec_options)?;
            api.register(system::server::restart::server_restart_options)?;
            api.register(system::server::config::server_config_options)?;
            api.register(system::server::config::server_config_console_options)?;
            api.register(system::server::backup::server_backup_options)?;
            // TODO remove in due time
            api.register(system::server::endpoint::server_endpoint_options)?;
        }
        api.register(system::server::version::server_version_get)?;
        api.register(system::server::spec::server_spec_get)?;
        api.register(system::server::restart::server_restart_post)?;
        api.register(system::server::config::server_config_get)?;
        api.register(system::server::config::server_config_put)?;
        api.register(system::server::config::server_config_console_get)?;
        api.register(system::server::backup::server_backup_post)?;
        // TODO remove in due time
        api.register(system::server::endpoint::server_endpoint_get)?;

        #[cfg(feature = "plus")]
        {
            // Server usage statistics
            if http_options {
                api.register(system::server::stats::server_stats_options)?;
            }
            api.register(system::server::stats::server_stats_get)?;
            // Bencher Cloud only
            if is_bencher_cloud {
                // TODO remove in due time
                api.register(system::server::stats::root_server_stats_post)?;
                api.register(system::server::stats::server_stats_post)?;
            }
        }

        #[cfg(feature = "plus")]
        {
            // Payments
            // Bencher Cloud only
            if is_bencher_cloud {
                if http_options {
                    // api.register(system::payments::payments_options)?;
                    api.register(system::payments::checkouts_options)?;
                }
                // api.register(system::payments::payments_post)?;
                api.register(system::payments::checkouts_post)?;
            }
        }

        Ok(())
    }
}
