pub mod custom;
pub mod rust;

use report::Report;

use crate::cli::benchmark::Output;
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
    pub fn convert(&self, output: Output) -> Result<Report, BencherError> {
        match &self {
            Adapter::Rust => rust::parse(output),
            Adapter::Custom(adapter) => custom::parse(adapter, output),
        }
    }
}
