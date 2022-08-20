use bencher_json::report::JsonNewBenchmarksMap;

use crate::{
    bencher::sub::run::perf::Output,
    BencherError,
};

pub fn parse(output: &Output) -> Result<JsonNewBenchmarksMap, BencherError> {
    serde_json::from_str(output.as_str()).map_err(BencherError::Serde)
}
