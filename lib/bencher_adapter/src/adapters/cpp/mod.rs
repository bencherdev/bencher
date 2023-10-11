pub mod catch2;
pub mod google;

use crate::{Adaptable, AdapterResults, Settings};
use catch2::AdapterCppCatch2;
use google::AdapterCppGoogle;

pub struct AdapterCpp;

impl Adaptable for AdapterCpp {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterCppCatch2::parse(input, settings)
            .or_else(|| AdapterCppGoogle::parse(input, settings))
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
    fn test_adapter_cpp_catch2() {
        let results = convert_file_path::<AdapterCpp>("./tool_output/cpp/catch2/four.txt");
        test_cpp_catch2::validate_adapter_cpp_catch2(&results);
    }

    #[test]
    fn test_adapter_cpp_google() {
        let results = convert_file_path::<AdapterCpp>("./tool_output/cpp/google/two.txt");
        test_cpp_google::validate_adapter_cpp_google(&results);
    }
}
