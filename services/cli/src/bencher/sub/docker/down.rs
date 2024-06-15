use crate::{
    bencher::sub::SubCmd,
    parser::docker::{CliContainer, CliDown},
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
    container: CliContainer,
}

impl From<CliDown> for Down {
    fn from(down: CliDown) -> Self {
        let CliDown { container } = down;
        Self {
            container: container.unwrap_or_default(),
        }
    }
}

impl SubCmd for Down {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;
        stop_containers(&docker, self.container).await?;
        cli_println!("🐰 Bencher Self-Hosted has been stopped.");
        Ok(())
    }
}

pub(super) async fn stop_containers(
    docker: &Docker,
    container: CliContainer,
) -> Result<(), DockerError> {
    if let CliContainer::All | CliContainer::Console = container {
        stop_container(docker, Container::Console).await?;
    }
    if let CliContainer::All | CliContainer::Api = container {
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
