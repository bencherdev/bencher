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

#[cfg(test)]
mod test_cpp {
    use super::AdapterCpp;
    use crate::adapters::{
        cpp::{catch2::test_cpp_catch2, google::test_cpp_google},
        test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_cpp_google() {
        let results = convert_file_path::<AdapterCpp>("./tool_output/cpp/google/two.txt");
        test_cpp_google::validate_adapter_cpp_google(results);
    }

    #[test]
    fn test_adapter_cpp_catch2() {
        let results = convert_file_path::<AdapterCpp>("./tool_output/cpp/catch2/four.txt");
        test_cpp_catch2::validate_adapter_cpp_catch2(results);
    }
}
