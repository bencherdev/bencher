pub mod bench;
pub mod criterion;
pub mod gungraun;
pub mod gungraun_json;
pub mod gungraun_stdout;
pub mod iai;

use self::{criterion::AdapterRustCriterion, gungraun::AdapterRustGungraun, iai::AdapterRustIai};
use crate::{Adaptable, AdapterResults, Settings};
use bench::AdapterRustBench;

pub struct AdapterRust;

impl Adaptable for AdapterRust {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustBench::parse(input, settings)
            .or_else(|| AdapterRustCriterion::parse(input, settings))
            .or_else(|| AdapterRustIai::parse(input, settings))
            .or_else(|| AdapterRustGungraun::parse(input, settings))
    }
}

#[cfg(test)]
mod test_rust {
    use super::AdapterRust;
    use crate::adapters::{
        rust::{
            bench::test_rust_bench, criterion::test_rust_criterion,
            gungraun_json::test_rust_gungraun_json, gungraun_stdout::test_rust_gungraun_stdout,
            iai::test_rust_iai,
        },
        test_util::convert_file_path,
    };

    #[test]
    fn adapter_rust_bench() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_bench(&results);
    }

    #[test]
    fn adapter_rust_criterion() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/criterion/many.txt");
        test_rust_criterion::validate_adapter_rust_criterion(&results);
    }

    #[test]
    fn adapter_rust_iai() {
        let results = convert_file_path::<AdapterRust>("./tool_output/rust/iai/two.txt");
        test_rust_iai::validate_adapter_rust_iai(&results);
    }

    #[test]
    fn adapter_rust_gungraun() {
        {
            let results = convert_file_path::<AdapterRust>(
                "./tool_output/rust/gungraun/json_pretty_one_callgrind.txt",
            );

            test_rust_gungraun_json::validate_adapter_rust_gungraun_json(&results);
        }

        {
            let results = convert_file_path::<AdapterRust>(
                "./tool_output/rust/gungraun/json_one_callgrind_diff.txt",
            );

            test_rust_gungraun_json::validate_adapter_rust_gungraun_json(&results);
        }

        {
            let results = convert_file_path::<AdapterRust>(
                "./tool_output/rust/gungraun/without-optional-metrics.txt",
            );

            test_rust_gungraun_stdout::validate_adapter_rust_gungraun_stdout(
                &results,
                &test_rust_gungraun_stdout::OptionalMetrics::default(),
            );
        }
    }
}
