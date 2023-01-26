pub mod bench;
pub mod criterion;

use self::criterion::AdapterRustCriterion;
use bench::AdapterRustBench;

use crate::{Adapter, AdapterError, AdapterResults, Settings};

pub struct AdapterRust;

impl Adapter for AdapterRust {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        let bench = AdapterRustBench::parse(input, settings)?;
        if !bench.is_empty() {
            return Ok(bench);
        }

        let criterion = AdapterRustCriterion::parse(input, settings)?;
        if !criterion.is_empty() {
            return Ok(criterion);
        }

        Ok(AdapterResults::default())
    }
}
