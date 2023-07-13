use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::system::server::CliServer, CliError};

mod backup;
mod config;
mod ping;
mod restart;
mod version;

#[derive(Debug)]
pub enum Server {
    Ping(ping::Ping),
    Version(version::Version),
    Restart(restart::Restart),
    Config(config::Config),
    Backup(backup::Backup),
}

impl TryFrom<CliServer> for Server {
    type Error = CliError;

    fn try_from(admin: CliServer) -> Result<Self, Self::Error> {
        Ok(match admin {
            CliServer::Ping(ping) => Self::Ping(ping.try_into()?),
            CliServer::Version(version) => Self::Version(version.try_into()?),
            CliServer::Restart(restart) => Self::Restart(restart.try_into()?),
            CliServer::Config(config) => Self::Config(config.try_into()?),
            CliServer::Backup(backup) => Self::Backup(backup.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Server {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Ping(ping) => ping.exec().await,
            Self::Version(version) => version.exec().await,
            Self::Restart(restart) => restart.exec().await,
            Self::Config(config) => config.exec().await,
            Self::Backup(backup) => backup.exec().await,
        }
    }
}
