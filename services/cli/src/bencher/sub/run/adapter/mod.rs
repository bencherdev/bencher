use bencher_json::report::{
    JsonNewAdapter,
    JsonNewBenchmarks,
};

use crate::{
    bencher::sub::run::benchmark::Output,
    cli::run::CliAdapter,
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

impl From<CliAdapter> for Adapter {
    fn from(adapter: CliAdapter) -> Self {
        match adapter {
            CliAdapter::Json => Adapter::Json,
            CliAdapter::RustCargoBench => Adapter::RustCargoBench,
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
            Adapter::Json => json::parse(output),
            Adapter::RustCargoBench => rust::parse(output),
        }
    }
}
