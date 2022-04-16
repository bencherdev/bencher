use crate::cli::clap::CliBackend;

pub mod repo;

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
