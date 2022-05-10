use crate::cli::clap::CliBackend;

use reports::Report;

pub mod repo;

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
    pub fn output(&self, report: Report) -> Result<String, BencherError> {
        match &self {
            Backend::Repo(repo) => repo.save(report),
        }
    }
}
