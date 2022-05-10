use crate::cli::clap::CliBackend;

use reports::Report;

use crate::BencherError;

#[derive(Debug)]
pub enum Backend {
    Url,
    Cloud,
    ToDo,
}

impl From<CliBackend> for Backend {
    fn from(backend: CliBackend) -> Self {
        Self::ToDo
    }
}

impl Backend {
    pub fn send(&self, report: Report) -> Result<(), BencherError> {
        todo!()
    }
}
