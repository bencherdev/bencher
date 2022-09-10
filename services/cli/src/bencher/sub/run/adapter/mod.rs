use bencher_json::report::{new::JsonBenchmarksMap, JsonAdapter};
use smart_default::SmartDefault;

use crate::{bencher::sub::run::perf::Output, cli::run::CliRunAdapter, CliError};

pub mod json;
pub mod rust;

#[derive(Clone, Copy, Debug, SmartDefault)]
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

impl From<Adapter> for JsonAdapter {
    fn from(adapter: Adapter) -> Self {
        match adapter {
            Adapter::Json => Self::Json,
            Adapter::RustTest => Self::RustTest,
            Adapter::RustBench => Self::RustBench,
        }
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<JsonBenchmarksMap, CliError> {
        match &self {
            Self::Json => json::parse(output),
            Self::RustTest => {
                todo!("cargo test -- -Z unstable-options --format json --report-time")
            },
            Self::RustBench => rust::parse(output),
        }
    }
}
