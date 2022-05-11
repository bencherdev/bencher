pub mod custom;
pub mod rust;

use reports::Metrics;

use crate::cli::benchmark::BenchmarkOutput;
use crate::BencherError;

/// Supported Adapters
#[derive(Clone, Debug)]
pub enum Adapter {
    /// Rust 🦀
    Rust,
    // Custom adapter
    Custom(String),
}

impl From<String> for Adapter {
    fn from(adapter: String) -> Self {
        match adapter.as_str() {
            "rust" => Adapter::Rust,
            _ => Adapter::Custom(adapter),
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: BenchmarkOutput) -> Result<Metrics, BencherError> {
        match &self {
            Adapter::Rust => rust::parse(output),
            Adapter::Custom(adapter) => custom::parse(adapter, output),
        }
    }
}
