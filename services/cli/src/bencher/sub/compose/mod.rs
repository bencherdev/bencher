pub mod container;
pub mod down;
pub mod logs;
pub mod network;
pub mod up;

use container::Container;

#[derive(thiserror::Error, Debug)]
pub enum DockerError {
    #[error(
        "Failed to connect to Docker daemon. Are you sure Docker is running?\nIf Docker is running, do you have the correct permissions?\nLearn more: https://docs.docker.com/engine/security/rootless/\nError: {0}"
    )]
    Daemon(bollard::errors::Error),
    #[error(
        "Failed to ping the Docker daemon. Are you sure Docker is running?\nIf Docker is running, do you have the correct permissions?\nLearn more: https://docs.docker.com/engine/security/rootless/\nError: {0}"
    )]
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
    #[error("Failed to create Docker network (`{network}`): {0}", network = network::NETWORK)]
    CreateNetwork(bollard::errors::Error),
    #[error("Failed to remove Docker network (`{network}`): {0}", network = network::NETWORK)]
    RemoveNetwork(bollard::errors::Error),
    #[error(
        "A Docker network named `{network}` already exists, and `bencher up` did not create it: it has no `{label}` label.\nRemove or rename that network, then run `bencher up` again.",
        network = network::NETWORK,
        label = network::NETWORK_LABEL
    )]
    ForeignNetwork,
    #[error("Failed to connect Docker container (`{container}`) to network (`{network}`): {err}", network = network::NETWORK)]
    ConnectNetwork {
        container: Container,
        err: bollard::errors::Error,
    },
}
