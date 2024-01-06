use crate::{bencher::sub::SubCmd, parser::docker::CliUp, CliError};
use bencher_json::BENCHER_API_PORT;
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    service::{HostConfig, PortBinding},
    Docker,
};

use super::{DockerError, BENCHER_API_CONTAINER, BENCHER_API_IMAGE};

#[derive(Debug, Clone)]
pub struct Up {}

impl From<CliUp> for Up {
    fn from(up: CliUp) -> Self {
        let CliUp {} = up;
        Self {}
    }
}

impl SubCmd for Up {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;

        super::stop_container(&docker, BENCHER_API_CONTAINER).await?;

        let options = Some(CreateContainerOptions {
            name: BENCHER_API_CONTAINER,
            platform: None,
        });
        let host_config = Some(HostConfig {
            port_bindings: Some(literally::hmap! {
                format!("{BENCHER_API_PORT}/tcp") => Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_owned()),
                    host_port: Some(BENCHER_API_PORT.to_string()),
                }]),
            }),
            publish_all_ports: Some(true),
            ..Default::default()
        });
        let config = Config {
            image: Some(BENCHER_API_IMAGE),
            host_config,
            exposed_ports: Some(literally::hmap! {
                "61016/tcp" => literally::hmap! {}
            }),
            ..Default::default()
        };
        docker
            .create_container(options, config)
            .await
            .map_err(|err| DockerError::CreateContainer {
                container: BENCHER_API_CONTAINER.to_owned(),
                err,
            })?;

        docker
            .start_container(BENCHER_API_CONTAINER, None::<StartContainerOptions<String>>)
            .await
            .map_err(|err| DockerError::StartContainer {
                container: BENCHER_API_CONTAINER.to_owned(),
                err,
            })?;

        Ok(())
    }
}
