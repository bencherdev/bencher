use crate::{bencher::sub::SubCmd, parser::docker::CliDown, CliError};
use bollard::Docker;

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

        super::stop_container(&docker, BENCHER_UI_CONTAINER).await?;
        super::stop_container(&docker, BENCHER_API_CONTAINER).await?;

        cli_println!("🐰 Bencher Self-Hosted has been stopped.");

        Ok(())
    }
}
