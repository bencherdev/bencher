use bencher_json::report::{
    new::JsonBenchmarksMap,
    JsonAdapter,
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
    RustTest,
    RustBench,
}

impl From<CliRunAdapter> for Adapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::RustTest => Self::RustTest,
            CliRunAdapter::RustBench => Self::RustBench,
        }
    }
}

impl Into<JsonAdapter> for Adapter {
    fn into(self) -> JsonAdapter {
        match self {
            Self::Json => JsonAdapter::Json,
            Self::RustTest => JsonAdapter::RustTest,
            Self::RustBench => JsonAdapter::RustBench,
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<JsonBenchmarksMap, BencherError> {
        match &self {
            Self::Json => json::parse(output),
            Self::RustTest => {
                todo!("cargo test -- -Z unstable-options --format json --report-time")
            },
            Self::RustBench => rust::parse(output),
        }
    }
}
