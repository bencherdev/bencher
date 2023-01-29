pub mod catch2;
pub mod google;

use crate::{Adapter, AdapterError, AdapterResults};
use catch2::AdapterCppCatch2;
use google::AdapterCppGoogle;

pub struct AdapterCpp;

impl Adapter for AdapterCpp {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let google = AdapterCppGoogle::parse(input);
        if google.is_ok() {
            return google;
        }

        let catch2 = AdapterCppCatch2::parse(input)?;
        if !catch2.is_empty() {
            return Ok(catch2);
        }

        Ok(AdapterResults::default())
    }
}
