pub mod rust;

use clap::ArgEnum;

/// Supported Languages
#[derive(ArgEnum, Clone, Debug)]
pub enum Adapter {
    /// Rust 🦀
    #[clap(name = "rust")]
    Rust,
    // Custom(String),
}
