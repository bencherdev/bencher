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

#[cfg(test)]
mod test_rust {
    use super::AdapterRust;
    use crate::adapters::{
        rust::{bench::test_rust_bench, criterion::test_rust_criterion},
        test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_magic_rust_bench() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_bench(results);
    }

    #[test]
    fn test_adapter_magic_rust_criterion() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/criterion/many.txt");
        test_rust_criterion::validate_adapter_rust_criterion(results);
    }
}
