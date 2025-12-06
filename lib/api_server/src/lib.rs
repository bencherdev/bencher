mod backup;
mod config;
mod restart;
mod root;
mod spec;
mod stats;
mod version;

pub use spec::{SPEC, SPEC_STR};

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Root
        if http_options {
            api_description.register(root::server_root_options)?;
        }
        api_description.register(root::server_root_get)?;

        // Server
        if http_options {
            api_description.register(version::server_version_options)?;
            api_description.register(spec::server_spec_options)?;
            api_description.register(restart::server_restart_options)?;
            api_description.register(config::server_config_options)?;
            api_description.register(config::server_config_console_options)?;
            api_description.register(backup::server_backup_options)?;
        }
        api_description.register(version::server_version_get)?;
        api_description.register(spec::server_spec_get)?;
        api_description.register(restart::server_restart_post)?;
        api_description.register(config::server_config_get)?;
        api_description.register(config::server_config_put)?;
        api_description.register(config::server_config_console_get)?;
        api_description.register(backup::server_backup_post)?;

        #[cfg(feature = "plus")]
        {
            // Server usage statistics
            if http_options {
                api_description.register(stats::server_stats_options)?;
            }
            api_description.register(stats::server_stats_get)?;

            // Bencher Cloud only
            if is_bencher_cloud {
                if http_options {
                    api_description.register(stats::server_startup_stats_options)?;
                }

                // TODO remove in due time
                api_description.register(stats::root_server_stats_post)?;
                api_description.register(stats::server_stats_post)?;

                api_description.register(stats::server_startup_stats_get)?;
            }
        }

        Ok(())
    }
}
