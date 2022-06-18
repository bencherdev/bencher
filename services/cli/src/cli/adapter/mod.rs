use crate::cli::benchmark::BenchmarkOutput;
use crate::cli::clap::CliAdapter;
use crate::BencherError;

pub mod json;
pub mod rust;

use reports::Metrics;

#[derive(Clone, Debug)]
pub enum Adapter {
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

pub fn map_adapter(adapter: Option<CliAdapter>) -> Result<Adapter, BencherError> {
    if let Some(adapter) = adapter {
        Ok(adapter.into())
    } else {
        Ok(Adapter::Json)
    }
}

impl Adapter {
    pub fn convert(&self, output: BenchmarkOutput) -> Result<Metrics, BencherError> {
        match &self {
            Adapter::Json => json::parse(output),
            Adapter::RustCargoBench => rust::parse(output),
        }
    }
}
