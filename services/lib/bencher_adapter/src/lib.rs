use bencher_json::project::report::new::JsonBenchmarksMap;

pub mod adapters;
pub mod error;

pub use adapters::{json::AdapterJson, magic::AdapterMagic, rust::AdapterRust};
pub use error::AdapterError;

pub trait Adapter {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError>;
}
