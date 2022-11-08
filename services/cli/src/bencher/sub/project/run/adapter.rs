use bencher_json::project::report::{new::JsonBenchmarksMap, JsonAdapter};

use crate::{bencher::sub::project::run::Output, cli::project::run::CliRunAdapter, CliError};

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
            Self::Json => todo!(),
            Self::RustTest => {
                todo!("cargo test -- -Z unstable-options --format json --report-time")
            },
            Self::RustBench => todo!(),
        }
    }
}
