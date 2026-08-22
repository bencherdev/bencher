use crate::{Adaptable, Settings, results::adapter_results::AdapterResults};

pub mod v0;
pub mod v1;

use v0::AdapterJsonV0;
use v1::AdapterJsonV1;

/// The `json` node of the adapter tree, over the `json_v0` and `json_v1` leaves.
///
/// Version detection is all or nothing per payload and needs no version field
/// and no magic key: each leaf requires every benchmark to take its own shape,
/// so a payload that mixes the two fails both leaves and therefore this node.
/// BMF v0 parses byte for byte as it always has.
pub struct AdapterJson;

impl Adaptable for AdapterJson {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsonV0::parse(input, settings).or_else(|| AdapterJsonV1::parse(input, settings))
    }
}

#[cfg(test)]
pub(crate) mod test_json {
    use pretty_assertions::assert_eq;

    use super::{
        AdapterJson,
        v0::{AdapterJsonV0, test_json_v0},
        v1::{AdapterJsonV1, test_json_v1},
    };
    use crate::{
        Adaptable as _, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path},
        results::adapter_results::{AdapterResults, BmfVersion},
    };

    fn convert_json(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{suffix}.json");
        convert_file_path::<AdapterJson>(&file_path)
    }

    #[test]
    fn adapter_json_latency() {
        let results = convert_json("latency");
        validate_adapter_json_latency(&results);
    }

    pub fn validate_adapter_json_latency(results: &AdapterResults) {
        test_json_v0::validate_adapter_json_latency(results);
    }

    #[test]
    fn adapter_json_dhat() {
        let results = convert_json("dhat");
        validate_adapter_json_dhat(&results);
    }

    pub fn validate_adapter_json_dhat(results: &AdapterResults) {
        test_json_v0::validate_adapter_json_dhat(results);
    }

    #[test]
    fn adapter_json_bmf_mixed() {
        let results = convert_json("bmf_mixed");
        test_json_v0::validate_adapter_json_bmf_mixed(&results);
    }

    /// Every v0 fixture parses identically through the node and through the leaf.
    #[test]
    fn adapter_json_v0_through_the_node() {
        for suffix in ["latency", "dhat", "bmf_mixed"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert_eq!(
                convert_file_path::<AdapterJson>(&file_path),
                convert_file_path::<AdapterJsonV0>(&file_path),
            );
        }
    }

    #[test]
    fn adapter_json_v1_latency() {
        let results = convert_json("v1_latency");
        test_json_v1::validate_adapter_json_v1_latency(&results);
    }

    #[test]
    fn adapter_json_v1_parameters() {
        let results = convert_json("v1_parameters");
        test_json_v1::validate_adapter_json_v1_parameters(&results);
    }

    #[test]
    fn adapter_json_v1_named() {
        let results = convert_json("v1_named");
        test_json_v1::validate_adapter_json_v1_named(&results);
    }

    #[test]
    fn adapter_json_v1_cap() {
        let results = convert_json("v1_cap");
        test_json_v1::validate_adapter_json_v1_cap(&results);
    }

    #[test]
    fn adapter_json_v1_canonical() {
        let results = convert_json("v1_canonical");
        test_json_v1::validate_adapter_json_v1_canonical(&results);
    }

    /// Every v1 fixture parses identically through the node and through the leaf.
    #[test]
    fn adapter_json_v1_through_the_node() {
        for suffix in [
            "v1_latency",
            "v1_parameters",
            "v1_named",
            "v1_cap",
            "v1_canonical",
        ] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert_eq!(
                convert_file_path::<AdapterJson>(&file_path),
                convert_file_path::<AdapterJsonV1>(&file_path),
            );
        }
    }

    /// A payload that mixes an object shaped benchmark and an array shaped one
    /// fails both leaves and therefore the node. This is the all or nothing rule.
    #[test]
    fn adapter_json_mixed_versions_fails() {
        let file_path = "./tool_output/json/report_mixed_versions.json";
        assert!(
            opt_convert_file_path::<AdapterJsonV0>(file_path, Settings::default()).is_none(),
            "expected a mixed version payload to fail json_v0"
        );
        assert!(
            opt_convert_file_path::<AdapterJsonV1>(file_path, Settings::default()).is_none(),
            "expected a mixed version payload to fail json_v1"
        );
        assert!(
            opt_convert_file_path::<AdapterJson>(file_path, Settings::default()).is_none(),
            "expected a mixed version payload to fail the json node"
        );
    }

    /// The node prefers v0, so an empty payload keeps parsing as it always has.
    #[test]
    fn adapter_json_empty_is_v0() {
        let results = AdapterJson::parse("{}", Settings::default()).unwrap();
        assert!(results.is_empty());
        assert_eq!(results.version, BmfVersion::V0);
    }
}
