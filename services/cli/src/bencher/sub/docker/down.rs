use crate::{
    bencher::sub::SubCmd,
    parser::docker::{CliDown, CliService},
    CliError,
};
use bollard::{
    container::{RemoveContainerOptions, StopContainerOptions},
    Docker,
};

use crate::cli_println;

use super::{Container, DockerError};

#[derive(Debug, Clone)]
pub struct Down {
    service: CliService,
}

impl From<CliDown> for Down {
    fn from(down: CliDown) -> Self {
        let CliDown { service } = down;
        Self { service }
    }
}

impl SubCmd for Down {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;
        // https://github.com/fussybeaver/bollard/issues/383
        docker.ping().await.map_err(DockerError::Ping)?;

        stop_containers(&docker, self.service).await?;
        cli_println!("🐰 Bencher Self-Hosted has been stopped.");

        Ok(())
    }
}

pub(super) async fn stop_containers(
    docker: &Docker,
    service: CliService,
) -> Result<(), DockerError> {
    if let CliService::All | CliService::Console = service {
        stop_container(docker, Container::Console).await?;
    }
    if let CliService::All | CliService::Api = service {
        stop_container(docker, Container::Api).await?;
    }
    Ok(())
}

pub(super) async fn stop_container(
    docker: &Docker,
    container: Container,
) -> Result<(), DockerError> {
    if docker
        .inspect_container(container.as_ref(), None)
        .await
        .is_ok()
    {
        cli_println!("Stopping existing `{container}` container...");
        let options = Some(StopContainerOptions { t: 5 });
        docker
            .stop_container(container.as_ref(), options)
            .await
            .map_err(|err| DockerError::StopContainer { container, err })?;

        cli_println!("Removing existing `{container}` container...");
        let options = Some(RemoveContainerOptions {
            force: true,
            ..Default::default()
        });
        docker
            .remove_container(container.as_ref(), options)
            .await
            .map_err(|err| DockerError::RemoveContainer { container, err })?;

        cli_println!("");
    }

    Ok(())
}
