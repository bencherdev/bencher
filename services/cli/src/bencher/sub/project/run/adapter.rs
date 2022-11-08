use bencher_adapter::Adapter;
use bencher_json::project::report::{new::JsonBenchmarksMap, JsonAdapter};

use crate::{bencher::sub::project::run::Output, cli::project::run::CliRunAdapter, CliError};

#[derive(Clone, Copy, Debug, Default)]
pub enum RunAdapter {
    #[default]
    Json,
    RustTest,
    RustBench,
}

impl From<CliRunAdapter> for RunAdapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::RustTest => Self::RustTest,
            CliRunAdapter::RustBench => Self::RustBench,
        }
    }
}

impl From<RunAdapter> for JsonAdapter {
    fn from(adapter: RunAdapter) -> Self {
        match adapter {
            RunAdapter::Json => Self::Json,
            RunAdapter::RustTest => Self::RustTest,
            RunAdapter::RustBench => Self::RustBench,
        }
    }
}

impl RunAdapter {
    pub fn convert(&self, output: &Output) -> Result<JsonBenchmarksMap, CliError> {
        let output_str = output.as_str();
        match &self {
            Self::Json => bencher_adapter::AdapterJson::convert(output_str),
            Self::RustTest => {
                todo!("cargo test -- -Z unstable-options --format json --report-time")
            },
            Self::RustBench => bencher_adapter::AdapterRustBench::convert(output_str),
        }
        .map_err(Into::into)
    }
}
