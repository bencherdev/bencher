// pub mod google;

// use self::criterion::AdapterRustCriterion;
use crate::{Adapter, AdapterError, AdapterResults};
// use bench::AdapterRustBench;

pub struct AdapterCpp;

impl Adapter for AdapterCpp {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        // let bench = AdapterRustBench::parse(input)?;
        // if !bench.is_empty() {
        //     return Ok(bench);
        // }

        // let criterion = AdapterRustCriterion::parse(input)?;
        // if !criterion.is_empty() {
        //     return Ok(criterion);
        // }

        Ok(AdapterResults::default())
    }
}
