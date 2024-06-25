pub mod container;
pub mod down;
pub mod logs;
pub mod up;

use container::Container;

#[derive(thiserror::Error, Debug)]
pub enum DockerError {
    #[error("Failed to connect to Docker daemon. Are you sure Docker is running?\nError: {0}")]
    Daemon(bollard::errors::Error),
    #[error("Failed to ping the Docker daemon. Are you sure Docker is running?\nError: {0}")]
    Ping(bollard::errors::Error),
    #[error("Failed to stop Docker container (`{container}`): {err}")]
    StopContainer {
        container: Container,
        err: bollard::errors::Error,
    },
    #[error("Failed to remove Docker container (`{container}`): {err}")]
    RemoveContainer {
        container: Container,
        err: bollard::errors::Error,
    },
    #[error("Failed to pull Docker image (`{image}`): {err}")]
    CreateImage {
        image: String,
        err: bollard::errors::Error,
    },
    #[error("Failed to create Docker container (`{container}`): {err}")]
    CreateContainer {
        container: Container,
        err: bollard::errors::Error,
    },
    #[error("Failed to start Docker container (`{container}`): {err}")]
    StartContainer {
        container: Container,
        err: bollard::errors::Error,
    },
}
