use bencher_json::project::report::new::JsonBenchmarksMap;
use nom::{
    character::complete::anychar,
    combinator::{eof, map_res},
    multi::many_till,
    IResult,
};

use crate::{Adapter, AdapterError};

pub struct AdapterJson;

impl Adapter for AdapterJson {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError> {
        parse_json(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

fn parse_json(input: &str) -> IResult<&str, JsonBenchmarksMap> {
    map_res(many_till(anychar, eof), |(char_array, _)| {
        serde_json::from_slice(&char_array.into_iter().map(|c| c as u8).collect::<Vec<u8>>())
    })(input)
}
