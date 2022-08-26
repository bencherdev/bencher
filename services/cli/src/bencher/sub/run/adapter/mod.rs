use bencher_json::report::{
    JsonAdapter,
    JsonNewBenchmarksMap,
};

use crate::{
    bencher::sub::run::perf::Output,
    cli::run::CliRunAdapter,
    BencherError,
};

pub mod json;
pub mod rust;

#[derive(Clone, Copy, Debug, Default)]
pub enum Adapter {
    #[default]
    Json,
    Rust,
}

impl From<CliRunAdapter> for Adapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::Rust => Self::Rust,
        }
    }
}

impl Into<JsonAdapter> for Adapter {
    fn into(self) -> JsonAdapter {
        match self {
            Self::Json => JsonAdapter::Json,
            Self::Rust => JsonAdapter::Rust,
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<JsonNewBenchmarksMap, BencherError> {
        match &self {
            Self::Json => json::parse(output),
            Self::Rust => rust::parse(output),
        }
    }
}
