use bencher_adapter::Adapter;
use bencher_json::project::report::{new::JsonBenchmarksMap, JsonAdapter};

use crate::{bencher::sub::project::run::Output, cli::project::run::CliRunAdapter, CliError};

#[derive(Clone, Copy, Debug, Default)]
pub enum RunAdapter {
    #[default]
    Magic,
    Json,
    Rust,
}

impl From<CliRunAdapter> for RunAdapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Magic => Self::Magic,
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::Rust => Self::Rust,
        }
    }
}

impl From<RunAdapter> for JsonAdapter {
    fn from(adapter: RunAdapter) -> Self {
        match adapter {
            RunAdapter::Magic => Self::Magic,
            RunAdapter::Json => Self::Json,
            RunAdapter::Rust => Self::Rust,
        }
    }
}

impl RunAdapter {
    pub fn convert(&self, output: &Output) -> Result<JsonBenchmarksMap, CliError> {
        let output_str = output.as_str();
        match &self {
            Self::Magic => bencher_adapter::AdapterMagic::convert(output_str),
            Self::Json => bencher_adapter::AdapterJson::convert(output_str),
            Self::Rust => bencher_adapter::AdapterRust::convert(output_str),
        }
        .map_err(Into::into)
    }
}
