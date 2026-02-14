// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod alerts;
mod allowed;
mod benchmarks;
mod branches;
mod jobs;
mod measures;
mod metrics;
mod perf;
mod plots;
mod projects;
mod reports;
mod testbeds;
mod thresholds;

mod macros;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    #[expect(clippy::too_many_lines, reason = "registration")]
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Projects
        // All of a projects's GET APIs are public if the project is public
        if http_options {
            api_description.register(projects::projects_options)?;
            api_description.register(projects::project_options)?;
        }
        api_description.register(projects::projects_get)?;
        api_description.register(projects::project_get)?;
        api_description.register(projects::project_patch)?;
        api_description.register(projects::project_delete)?;

        // Project Permission
        if http_options {
            api_description.register(allowed::proj_allowed_options)?;
        }
        api_description.register(allowed::proj_allowed_get)?;

        // Reports
        if http_options {
            api_description.register(reports::proj_reports_options)?;
            api_description.register(reports::proj_report_options)?;
        }
        api_description.register(reports::proj_report_post)?;
        api_description.register(reports::proj_reports_get)?;
        api_description.register(reports::proj_report_get)?;
        api_description.register(reports::proj_report_delete)?;

        // Jobs
        #[cfg(feature = "plus")]
        {
            if http_options {
                api_description.register(jobs::proj_jobs_options)?;
                api_description.register(jobs::proj_job_options)?;
            }
            api_description.register(jobs::proj_jobs_get)?;
            api_description.register(jobs::proj_job_get)?;
        }

        // Perf
        if http_options {
            api_description.register(perf::proj_perf_options)?;
        }
        api_description.register(perf::proj_perf_get)?;

        // Perf Image
        if http_options {
            api_description.register(perf::img::proj_perf_img_options)?;
        }
        api_description.register(perf::img::proj_perf_img_get)?;

        // Plots
        if http_options {
            api_description.register(plots::proj_plots_options)?;
            api_description.register(plots::proj_plot_options)?;
        }
        api_description.register(plots::proj_plots_get)?;
        api_description.register(plots::proj_plot_post)?;
        api_description.register(plots::proj_plot_get)?;
        api_description.register(plots::proj_plot_patch)?;
        api_description.register(plots::proj_plot_delete)?;

        // Branches
        if http_options {
            api_description.register(branches::proj_branches_options)?;
            api_description.register(branches::proj_branch_options)?;
        }
        api_description.register(branches::proj_branches_get)?;
        api_description.register(branches::proj_branch_post)?;
        api_description.register(branches::proj_branch_get)?;
        api_description.register(branches::proj_branch_patch)?;
        api_description.register(branches::proj_branch_delete)?;

        // Testbeds
        if http_options {
            api_description.register(testbeds::proj_testbeds_options)?;
            api_description.register(testbeds::proj_testbed_options)?;
        }
        api_description.register(testbeds::proj_testbeds_get)?;
        api_description.register(testbeds::proj_testbed_post)?;
        api_description.register(testbeds::proj_testbed_get)?;
        api_description.register(testbeds::proj_testbed_patch)?;
        api_description.register(testbeds::proj_testbed_delete)?;

        // Benchmarks
        if http_options {
            api_description.register(benchmarks::proj_benchmarks_options)?;
            api_description.register(benchmarks::proj_benchmark_options)?;
        }
        api_description.register(benchmarks::proj_benchmarks_get)?;
        api_description.register(benchmarks::proj_benchmark_post)?;
        api_description.register(benchmarks::proj_benchmark_get)?;
        api_description.register(benchmarks::proj_benchmark_patch)?;
        api_description.register(benchmarks::proj_benchmark_delete)?;

        // Measures
        if http_options {
            api_description.register(measures::proj_measures_options)?;
            api_description.register(measures::proj_measure_options)?;
        }
        api_description.register(measures::proj_measures_get)?;
        api_description.register(measures::proj_measure_post)?;
        api_description.register(measures::proj_measure_get)?;
        api_description.register(measures::proj_measure_patch)?;
        api_description.register(measures::proj_measure_delete)?;

        // Metrics
        if http_options {
            api_description.register(metrics::proj_metric_options)?;
        }
        api_description.register(metrics::proj_metric_get)?;

        // Thresholds
        if http_options {
            api_description.register(thresholds::proj_thresholds_options)?;
            api_description.register(thresholds::proj_threshold_options)?;
        }
        api_description.register(thresholds::proj_thresholds_get)?;
        api_description.register(thresholds::proj_threshold_post)?;
        api_description.register(thresholds::proj_threshold_get)?;
        api_description.register(thresholds::proj_threshold_put)?;
        api_description.register(thresholds::proj_threshold_delete)?;

        // Threshold Alerts
        if http_options {
            api_description.register(alerts::proj_alerts_options)?;
            api_description.register(alerts::proj_alert_options)?;
        }
        api_description.register(alerts::proj_alerts_get)?;
        api_description.register(alerts::proj_alert_get)?;
        api_description.register(alerts::proj_alert_patch)?;

        Ok(())
    }
}
