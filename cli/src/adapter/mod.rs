pub mod custom;
mod report;
pub mod rust;

pub use report::Report;

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
