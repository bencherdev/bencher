pub mod bench;
pub mod criterion;

use self::criterion::AdapterRustCriterion;
use crate::{Adapter, AdapterError, AdapterResults};
use bench::AdapterRustBench;

pub struct AdapterRust;

impl Adapter for AdapterRust {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let bench = AdapterRustBench::parse(input)?;
        if !bench.is_empty() {
            return Ok(bench);
        }

        let criterion = AdapterRustCriterion::parse(input)?;
        if !criterion.is_empty() {
            return Ok(criterion);
        }

        Ok(AdapterResults::default())
    }
}
