use bencher_json::BmfVersion;

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
///
/// The report payload's `bmf_version` reorders the two attempts and nothing else.
/// At version 1 the v1 leaf is tried first and the v0 leaf catches what it does
/// not claim; at version 0, which is also what an absent key means, the order is
/// the other way around. Either order lands on the same leaf for any payload only
/// one leaf claims, so the key is a statement about the payload rather than a
/// filter on it.
pub struct AdapterJson;

impl Adaptable for AdapterJson {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        if settings.bmf_version == BmfVersion::V1 {
            AdapterJsonV1::parse(input, settings).or_else(|| AdapterJsonV0::parse(input, settings))
        } else {
            AdapterJsonV0::parse(input, settings).or_else(|| AdapterJsonV1::parse(input, settings))
        }
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
    use bencher_json::BmfVersion;

    use crate::{
        Adaptable as _, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path},
        results::adapter_results::AdapterResults,
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

    /// A v1 payload whose parameter set breaks a bound is claimed by no leaf.
    ///
    /// Observed for this fixture, whose set carries one key over the cap: the v0
    /// leaf returns `None`, the v1 leaf returns `None`, and so does this node.
    /// The v0 fallback is not a soft landing. It never sees a shape it
    /// recognizes, so the report fails to parse and the run is rejected rather
    /// than degrading to v0 and silently dropping the parameters.
    #[test]
    fn adapter_json_out_of_bounds_parameters_fails_every_leaf() {
        let file_path = "./tool_output/json/report_v1_bad_parameters.json";
        assert!(
            opt_convert_file_path::<AdapterJsonV0>(file_path, Settings::default()).is_none(),
            "expected the v0 leaf to reject an out of bounds parameter set"
        );
        assert!(
            opt_convert_file_path::<AdapterJsonV1>(file_path, Settings::default()).is_none(),
            "expected the v1 leaf to reject an out of bounds parameter set"
        );
        assert!(
            opt_convert_file_path::<AdapterJson>(file_path, Settings::default()).is_none(),
            "expected the json node to reject an out of bounds parameter set"
        );
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

    /// Every JSON fixture, whichever leaf claims it and whether any leaf does.
    pub const JSON_FIXTURES: [&str; 11] = [
        "latency",
        "dhat",
        "bmf_mixed",
        "v1_latency",
        "v1_parameters",
        "v1_named",
        "v1_cap",
        "v1_cap_permuted",
        "v1_canonical",
        "mixed_versions",
        "v1_bad_parameters",
    ];

    pub fn version_settings(bmf_version: BmfVersion) -> Settings {
        Settings::new(None, bmf_version)
    }

    /// An absent `bmf_version` is version 0, so the two parse to the same bytes.
    #[test]
    fn adapter_json_absent_version_is_version_0() {
        for suffix in JSON_FIXTURES {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert_eq!(
                opt_convert_file_path::<AdapterJson>(&file_path, Settings::default()),
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(BmfVersion::V0)),
                "{suffix}"
            );
        }
    }

    /// Version 1 reorders the two attempts and changes no outcome.
    ///
    /// Every fixture here is claimed by at most one leaf, so trying the leaves in
    /// either order lands on the same one. That is the whole promise of the key in
    /// this layer: a v1 payload no longer waits behind a v0 attempt, a v0 payload
    /// is still read as v0, and a payload no leaf claims is still rejected.
    #[test]
    fn adapter_json_version_1_reorders_the_attempts_and_nothing_else() {
        for suffix in JSON_FIXTURES {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert_eq!(
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(BmfVersion::V1)),
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(BmfVersion::V0)),
                "{suffix}"
            );
        }
    }

    /// A v0 payload still ingests at version 1, through the v0 fallback.
    ///
    /// Version 1 is a statement about the payload, not a filter on it, so this
    /// layer refuses no shape it accepted before.
    #[test]
    fn adapter_json_version_1_does_not_refuse_a_v0_payload() {
        for suffix in ["latency", "dhat", "bmf_mixed"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            let results =
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(BmfVersion::V1))
                    .unwrap_or_else(|| panic!("expected {suffix} to ingest at version 1"));
            assert_eq!(results, convert_file_path::<AdapterJsonV0>(&file_path));
            assert_eq!(results.version, BmfVersion::V0);
        }
    }

    /// The empty payload is the one payload both leaves claim, so it is the one
    /// payload that shows which leaf the node tried first.
    #[test]
    fn adapter_json_empty_is_v1_at_version_1() {
        let results = AdapterJson::parse("{}", version_settings(BmfVersion::V1)).unwrap();
        assert!(results.is_empty());
        assert_eq!(results.version, BmfVersion::V1);
    }

    /// An explicitly named leaf is an exact statement, so the key does not move it.
    ///
    /// The discriminating case is a leaf pointed at the other version's shape: if
    /// `bmf_version` could override the named leaf, these would parse.
    #[test]
    fn adapter_json_leaves_ignore_the_version() {
        let settings = version_settings(BmfVersion::V1);
        for suffix in ["v1_latency", "v1_parameters", "v1_named", "v1_canonical"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert!(
                opt_convert_file_path::<AdapterJsonV0>(&file_path, settings).is_none(),
                "expected the json_v0 leaf to reject the v1 payload {suffix} at version 1"
            );
        }
        for suffix in ["latency", "dhat", "bmf_mixed"] {
            let file_path = format!("./tool_output/json/report_{suffix}.json");
            assert!(
                opt_convert_file_path::<AdapterJsonV1>(&file_path, settings).is_none(),
                "expected the json_v1 leaf to reject the v0 payload {suffix} at version 1"
            );
            // And the leaf that does claim it reads it exactly as it always has.
            assert_eq!(
                opt_convert_file_path::<AdapterJsonV0>(&file_path, settings),
                Some(convert_file_path::<AdapterJsonV0>(&file_path)),
                "{suffix}"
            );
        }
    }
}
