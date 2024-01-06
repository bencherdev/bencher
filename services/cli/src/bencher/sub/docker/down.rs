use crate::{bencher::sub::SubCmd, parser::docker::CliDown, CliError};
use bollard::{
    container::{RemoveContainerOptions, StopContainerOptions},
    Docker,
};

use crate::cli_println;

use super::{DockerError, BENCHER_API_CONTAINER, BENCHER_UI_CONTAINER};

#[derive(Debug, Clone)]
pub struct Down {}

impl From<CliDown> for Down {
    fn from(down: CliDown) -> Self {
        let CliDown {} = down;
        Self {}
    }
}

impl SubCmd for Down {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;

        stop_container(&docker, BENCHER_UI_CONTAINER).await?;
        stop_container(&docker, BENCHER_API_CONTAINER).await?;

        cli_println!("🐰 Bencher Self-Hosted has been stopped.");

        Ok(())
    }
}

pub async fn stop_container(docker: &Docker, container: &str) -> Result<(), DockerError> {
    if docker.inspect_container(container, None).await.is_ok() {
        cli_println!("Stopping existing `{container}` container...");
        let options = Some(StopContainerOptions { t: 5 });
        docker
            .stop_container(container, options)
            .await
            .map_err(|err| DockerError::StopContainer {
                container: container.to_owned(),
                err,
            })?;

        cli_println!("Removing existing `{container}` container...");
        let options = Some(RemoveContainerOptions {
            force: true,
            ..Default::default()
        });
        docker
            .remove_container(container, options)
            .await
            .map_err(|err| DockerError::RemoveContainer {
                container: container.to_owned(),
                err,
            })?;
    }

    Ok(())
}
