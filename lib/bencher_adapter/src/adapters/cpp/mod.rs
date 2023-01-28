pub mod google;

// use self::criterion::AdapterRustCriterion;
use crate::{Adapter, AdapterError, AdapterResults};
use google::AdapterCppGoogle;

pub struct AdapterCpp;

impl Adapter for AdapterCpp {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let google = AdapterCppGoogle::parse(input);
        if google.is_ok() {
            return google;
        }

        // let criterion = AdapterRustCriterion::parse(input)?;
        // if !criterion.is_empty() {
        //     return Ok(criterion);
        // }

        Ok(AdapterResults::default())
    }
}
