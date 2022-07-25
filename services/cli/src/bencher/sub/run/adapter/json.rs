use bencher_json::JsonBenchmarks;

use crate::{
    bencher::sub::run::benchmark::Output,
    BencherError,
};

pub fn parse(output: &Output) -> Result<JsonBenchmarks, BencherError> {
    serde_json::from_str(output.as_str()).map_err(BencherError::Serde)
}
