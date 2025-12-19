pub mod gungraun_json;
pub mod gungraun_text;

use crate::{
    adapters::rust::gungraun::{
        gungraun_json::AdapterRustGungraunJson, gungraun_text::AdapterRustGungraunText,
    },
    Adaptable, AdapterResults, Settings,
};

pub struct AdapterRustGungraun;

impl Adaptable for AdapterRustGungraun {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustGungraunJson::parse(input, settings)
            .or_else(|| AdapterRustGungraunText::parse(input, settings))
    }
}

#[cfg(test)]
mod test_rust {
    use super::AdapterRustGungraun;
    use crate::adapters::test_util::convert_file_path;

    #[test]
    fn adapter_rust_gungraun() {
        let results = convert_file_path::<AdapterRustGungraun>(
            "./tool_output/rust/gungraun/without-optional-metrics.txt",
        );

        // test_rust_gungraun::validate_adapter_rust_gungraun(
        //     &results,
        //     &test_rust_gungraun::OptionalMetrics::default(),
        // );
    }
}
