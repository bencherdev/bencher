pub mod adapters;
pub mod error;
pub mod results;

use adapters::rust::{bench::AdapterRustBench, criterion::AdapterRustCriterion};
pub use adapters::{json::AdapterJson, magic::AdapterMagic, rust::AdapterRust};
use bencher_json::project::report::{JsonAdapter, JsonReportSettings};
pub use error::AdapterError;
pub use results::{adapter_results::AdapterResults, AdapterResultsArray};

pub trait Adapter {
    fn convert(&self, input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        Self::parse(input, settings)
    }

    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError>;
}

impl Adapter for JsonAdapter {
    fn convert(&self, input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        match self {
            JsonAdapter::Magic => AdapterMagic::parse(input, settings),
            JsonAdapter::Json => AdapterJson::parse(input, settings),
            JsonAdapter::Rust => AdapterRust::parse(input, settings),
            JsonAdapter::RustBench => AdapterRustBench::parse(input, settings),
            JsonAdapter::RustCriterion => AdapterRustCriterion::parse(input, settings),
        }
    }

    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        AdapterMagic::parse(input, settings)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Settings {
    pub allow_failure: bool,
}

impl From<JsonReportSettings> for Settings {
    fn from(settings: JsonReportSettings) -> Self {
        let JsonReportSettings { allow_failure, .. } = settings;
        Self {
            allow_failure: allow_failure.unwrap_or_default(),
        }
    }
}
