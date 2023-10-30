use async_trait::async_trait;

use crate::{bencher::sub::SubCmd, parser::system::server::CliServer, CliError};

mod backup;
mod config;
mod endpoint;
mod ping;
mod restart;
mod spec;
mod stats;
mod version;

#[derive(Debug)]
pub enum Server {
    Ping(ping::Ping),
    Version(version::Version),
    Spec(spec::Spec),
    Endpoint(endpoint::Endpoint),
    Restart(restart::Restart),
    Config(config::Config),
    Backup(backup::Backup),
    Stats(stats::ServerStats),
}

impl TryFrom<CliServer> for Server {
    type Error = CliError;

    fn try_from(admin: CliServer) -> Result<Self, Self::Error> {
        Ok(match admin {
            CliServer::Ping(ping) => Self::Ping(ping.try_into()?),
            CliServer::Version(version) => Self::Version(version.try_into()?),
            CliServer::Spec(spec) => Self::Spec(spec.try_into()?),
            CliServer::Endpoint(endpoint) => Self::Endpoint(endpoint.try_into()?),
            CliServer::Restart(restart) => Self::Restart(restart.try_into()?),
            CliServer::Config(config) => Self::Config(config.try_into()?),
            CliServer::Backup(backup) => Self::Backup(backup.try_into()?),
            CliServer::Stats(stats) => Self::Stats(stats.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Server {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Ping(ping) => ping.exec().await,
            Self::Version(version) => version.exec().await,
            Self::Spec(spec) => spec.exec().await,
            Self::Endpoint(endpoint) => endpoint.exec().await,
            Self::Restart(restart) => restart.exec().await,
            Self::Config(config) => config.exec().await,
            Self::Backup(backup) => backup.exec().await,
            Self::Stats(stats) => stats.exec().await,
        }
    }
}
