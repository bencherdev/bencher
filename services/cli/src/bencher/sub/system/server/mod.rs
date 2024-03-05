use crate::{bencher::sub::SubCmd, parser::system::server::CliServer, CliError};

mod backup;
mod config;
mod restart;
mod spec;
#[cfg(feature = "plus")]
mod stats;
mod version;

#[derive(Debug)]
pub enum Server {
    Version(version::Version),
    Spec(spec::Spec),
    Restart(restart::Restart),
    Config(config::Config),
    Backup(backup::Backup),
    #[cfg(feature = "plus")]
    Stats(stats::ServerStats),
}

impl TryFrom<CliServer> for Server {
    type Error = CliError;

    fn try_from(admin: CliServer) -> Result<Self, Self::Error> {
        Ok(match admin {
            CliServer::Version(version) => Self::Version(version.try_into()?),
            CliServer::Spec(spec) => Self::Spec(spec.try_into()?),
            CliServer::Restart(restart) => Self::Restart(restart.try_into()?),
            CliServer::Config(config) => Self::Config(config.try_into()?),
            CliServer::Backup(backup) => Self::Backup(backup.try_into()?),
            #[cfg(feature = "plus")]
            CliServer::Stats(stats) => Self::Stats(stats.try_into()?),
        })
    }
}

impl SubCmd for Server {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Version(version) => version.exec().await,
            Self::Spec(spec) => spec.exec().await,
            Self::Restart(restart) => restart.exec().await,
            Self::Config(config) => config.exec().await,
            Self::Backup(backup) => backup.exec().await,
            #[cfg(feature = "plus")]
            Self::Stats(stats) => stats.exec().await,
        }
    }
}
