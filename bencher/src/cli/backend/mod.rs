use crate::cli::clap::CliBackend;

pub mod repo;

use crate::cli::adapter::Report;
use crate::BencherError;
use repo::Repo;

#[derive(Debug)]
pub enum Backend {
    Repo(Repo),
}

impl From<CliBackend> for Backend {
    fn from(backend: CliBackend) -> Self {
        match backend {
            CliBackend::Repo(repo) => Backend::Repo(Repo::from(repo)),
        }
    }
}

impl Backend {
    pub fn output(&self, report: Report) -> Result<(), BencherError> {
        match &self {
            Backend::Repo(git) => git.save(report),
        }
    }
}
