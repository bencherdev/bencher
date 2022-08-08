use bencher_json::report::{
    JsonNewAdapter,
    JsonNewBenchmarks,
};

use crate::{
    bencher::sub::run::benchmark::Output,
    cli::run::CliRunAdapter,
    BencherError,
};

pub mod json;
pub mod rust;

#[derive(Clone, Copy, Debug, Default)]
pub enum Adapter {
    #[default]
    Json,
    RustCargoBench,
}

impl From<CliRunAdapter> for Adapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::RustCargoBench => Self::RustCargoBench,
        }
    }
}

impl Into<JsonNewAdapter> for Adapter {
    fn into(self) -> JsonNewAdapter {
        match self {
            Self::Json => JsonNewAdapter::Json,
            Self::RustCargoBench => JsonNewAdapter::RustCargoBench,
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<JsonNewBenchmarks, BencherError> {
        match &self {
            Self::Json => json::parse(output),
            Self::RustCargoBench => rust::parse(output),
        }
    }
}
