use crate::{parser::up::CliUp, CliError};
use bollard::Docker;

use super::SubCmd;

#[derive(Debug, Clone)]
pub struct Up {}

#[derive(thiserror::Error, Debug)]
pub enum UpError {
    #[error("Failed to connect to Docker daemon: {0}")]
    DockerDaemon(bollard::errors::Error),
}

impl From<CliUp> for Up {
    fn from(up: CliUp) -> Self {
        let CliUp {} = up;
        Self {}
    }
}

impl SubCmd for Up {
    async fn exec(&self) -> Result<(), CliError> {
        let _docker = Docker::connect_with_local_defaults().map_err(UpError::DockerDaemon)?;
        Ok(())
    }
}
