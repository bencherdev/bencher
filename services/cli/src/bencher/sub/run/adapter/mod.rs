use bencher_json::{
    JsonAdapter,
    JsonBenchmarks,
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

impl Into<JsonAdapter> for Adapter {
    fn into(self) -> JsonAdapter {
        match self {
            Self::Json => JsonAdapter::Json,
            Self::RustCargoBench => JsonAdapter::RustCargoBench,
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<JsonBenchmarks, BencherError> {
        match &self {
            Adapter::Json => json::parse(output),
            Adapter::RustCargoBench => rust::parse(output),
        }
    }
}
