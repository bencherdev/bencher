pub mod bench;

use crate::{Adapter, AdapterError, AdapterResults};
use bench::AdapterGoBench;

pub struct AdapterGo;

impl Adapter for AdapterGo {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let bench = AdapterGoBench::parse(input)?;
        if !bench.is_empty() {
            return Ok(bench);
        }

        Ok(AdapterResults::default())
    }
}
