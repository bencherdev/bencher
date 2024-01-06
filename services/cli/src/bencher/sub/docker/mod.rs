pub mod down;
pub mod logs;
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
