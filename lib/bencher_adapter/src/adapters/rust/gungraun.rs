use crate::Adaptable;

use super::{gungraun_json::AdapterRustGungraunJson, gungraun_stdout::AdapterRustGungraunStdout};

pub struct AdapterRustGungraun;

impl Adaptable for AdapterRustGungraun {
    fn parse(input: &str, settings: crate::Settings) -> Option<crate::AdapterResults> {
        AdapterRustGungraunJson::parse(input, settings)
            .or_else(|| AdapterRustGungraunStdout::parse(input, settings))
    }
}

#[cfg(test)]
pub(crate) mod test_rust_gungraun {
    use crate::adapters::{
        rust::{
            gungraun_json::test_rust_gungraun_json::validate_adapter_rust_gungraun_json,
            gungraun_stdout::{AdapterRustGungraunStdout, test_rust_gungraun_stdout},
        },
        test_util::convert_file_path,
    };

    use super::AdapterRustGungraunJson;

    #[test]
    fn json() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_callgrind_diff.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn stdout() {
        let results = convert_file_path::<AdapterRustGungraunStdout>(
            "./tool_output/rust/gungraun/without-optional-metrics.txt",
        );

        test_rust_gungraun_stdout::validate_adapter_rust_gungraun_stdout(
            &results,
            &test_rust_gungraun_stdout::OptionalMetrics::default(),
        );
    }
}
