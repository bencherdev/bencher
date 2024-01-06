use bollard::{
    container::{RemoveContainerOptions, StopContainerOptions},
    Docker,
};

use crate::cli_println;

pub mod down;
pub mod up;

const BENCHER_API_CONTAINER: &str = "bencher_api_local";
const BENCHER_API_IMAGE: &str = "ghcr.io/bencherdev/bencher-api-local:latest";

const BENCHER_UI_CONTAINER: &str = "bencher_ui";
const BENCHER_UI_IMAGE: &str = "ghcr.io/bencherdev/bencher-ui:latest";

#[derive(thiserror::Error, Debug)]
pub enum DockerError {
    #[error("Failed to connect to Docker daemon. Are you sure Docker is running?\nError: {0}")]
    Daemon(bollard::errors::Error),
    #[error("Failed to stop Docker container (`{container}`): {err}")]
    StopContainer {
        container: String,
        err: bollard::errors::Error,
    },
    #[error("Failed to remove Docker container (`{container}`): {err}")]
    RemoveContainer {
        container: String,
        err: bollard::errors::Error,
    },
    #[error("Failed to create Docker container (`{container}`): {err}")]
    CreateContainer {
        container: String,
        err: bollard::errors::Error,
    },
    #[error("Failed to start Docker container (`{container}`): {err}")]
    StartContainer {
        container: String,
        err: bollard::errors::Error,
    },
}

async fn stop_container(docker: &Docker, container: &str) -> Result<(), DockerError> {
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
