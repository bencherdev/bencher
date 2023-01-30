pub mod adapters;
pub mod error;
pub mod results;

pub use adapters::{cpp::AdapterCpp, json::AdapterJson, magic::AdapterMagic, rust::AdapterRust};
use adapters::{
    cpp::{catch2::AdapterCppCatch2, google::AdapterCppGoogle},
    go::{bench::AdapterGoBench, AdapterGo},
    rust::{bench::AdapterRustBench, criterion::AdapterRustCriterion},
};
use bencher_json::project::report::JsonAdapter;
pub use error::AdapterError;
pub use results::{adapter_results::AdapterResults, AdapterResultsArray};

pub trait Adapter {
    fn convert(&self, input: &str) -> Result<AdapterResults, AdapterError> {
        Self::parse(input)
    }

    fn parse(input: &str) -> Result<AdapterResults, AdapterError>;
}

impl Adapter for JsonAdapter {
    fn convert(&self, input: &str) -> Result<AdapterResults, AdapterError> {
        match self {
            JsonAdapter::Magic => AdapterMagic::parse(input),
            JsonAdapter::Json => AdapterJson::parse(input),
            JsonAdapter::Cpp => AdapterCpp::parse(input),
            JsonAdapter::CppGoogle => AdapterCppGoogle::parse(input),
            JsonAdapter::CppCatch2 => AdapterCppCatch2::parse(input),
            JsonAdapter::Go => AdapterGo::parse(input),
            JsonAdapter::GoBench => AdapterGoBench::parse(input),
            JsonAdapter::Rust => AdapterRust::parse(input),
            JsonAdapter::RustBench => AdapterRustBench::parse(input),
            JsonAdapter::RustCriterion => AdapterRustCriterion::parse(input),
        }
    }

    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        AdapterMagic::parse(input)
    }
}
