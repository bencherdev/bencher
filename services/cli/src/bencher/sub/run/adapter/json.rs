use bencher_json::report::new::JsonBenchmarksMap;

use crate::{bencher::sub::run::perf::Output, CliError};

pub fn parse(output: &Output) -> Result<JsonBenchmarksMap, CliError> {
    serde_json::from_str(output.as_str()).map_err(CliError::Serde)
}
