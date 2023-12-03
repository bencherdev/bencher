pub mod bench;
pub mod criterion;
pub mod iai;

use self::{criterion::AdapterRustCriterion, iai::AdapterRustIai};
use crate::{Adaptable, AdapterResults, Settings};
use bench::AdapterRustBench;

pub struct AdapterRust;

impl Adaptable for AdapterRust {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustBench::parse(input, settings)
            .or_else(|| AdapterRustCriterion::parse(input, settings))
            .or_else(|| AdapterRustIai::parse(input, settings))
    }
}

#[cfg(test)]
mod test_rust {
    use super::AdapterRust;
    use crate::adapters::{
        rust::{bench::test_rust_bench, criterion::test_rust_criterion, iai::test_rust_iai},
        test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_rust_bench() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_bench(&results);
    }

    #[test]
    fn test_adapter_rust_criterion() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/criterion/many.txt");
        test_rust_criterion::validate_adapter_rust_criterion(&results);
    }

    #[test]
    fn test_adapter_rust_iai() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/iai/two.txt");
        test_rust_iai::validate_adapter_rust_iai(&results);
    }
}
