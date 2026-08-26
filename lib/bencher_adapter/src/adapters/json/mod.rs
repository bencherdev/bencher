use bencher_json::BmfVersion;

use crate::{Adaptable, Settings, results::adapter_results::AdapterResults};

pub mod v0;
pub mod v1;

use v0::AdapterJsonV0;
use v1::AdapterJsonV1;

/// The `json` node of the adapter tree, over the `json_v0` and `json_v1` leaves.
///
/// The payload's declared `bmf_version` picks the one leaf this node parses with,
/// and an absent key is version 0. A payload that leaf does not claim fails the node.
pub struct AdapterJson;

impl Adaptable for AdapterJson {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        if settings.bmf_version == BmfVersion::V1 {
            AdapterJsonV1::parse(input, settings)
        } else {
            AdapterJsonV0::parse(input, settings)
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

    /// The fixtures the v0 leaf claims.
    pub const V0_FIXTURES: [&str; 3] = ["latency", "dhat", "bmf_mixed"];
    /// The fixtures the v1 leaf claims.
    pub const V1_FIXTURES: [&str; 6] = [
        "v1_latency",
        "v1_parameters",
        "v1_named",
        "v1_cap",
        "v1_cap_permuted",
        "v1_canonical",
    ];
    /// The fixtures no leaf claims.
    pub const UNCLAIMED_FIXTURES: [&str; 2] = ["mixed_versions", "v1_bad_parameters"];
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

    pub fn fixture_path(suffix: &str) -> String {
        format!("./tool_output/json/report_{suffix}.json")
    }

    pub fn version_settings(bmf_version: BmfVersion) -> Settings {
        Settings::new(None, bmf_version)
    }

    fn convert_json(suffix: &str) -> AdapterResults {
        convert_file_path::<AdapterJson>(&fixture_path(suffix))
    }

    fn convert_json_v1(suffix: &str) -> AdapterResults {
        opt_convert_file_path::<AdapterJson>(
            &fixture_path(suffix),
            version_settings(BmfVersion::V1),
        )
        .unwrap_or_else(|| panic!("expected {suffix} to parse at version 1"))
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

    /// A v1 payload whose parameter set breaks a bound is claimed by no leaf, so
    /// the report fails to parse rather than degrading and dropping the parameters.
    #[test]
    fn adapter_json_out_of_bounds_parameters_fails_every_leaf() {
        let file_path = fixture_path("v1_bad_parameters");
        assert!(
            opt_convert_file_path::<AdapterJsonV0>(&file_path, Settings::default()).is_none(),
            "expected the v0 leaf to reject an out of bounds parameter set"
        );
        assert!(
            opt_convert_file_path::<AdapterJsonV1>(&file_path, Settings::default()).is_none(),
            "expected the v1 leaf to reject an out of bounds parameter set"
        );
        for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
            assert!(
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(bmf_version))
                    .is_none(),
                "expected the json node to reject an out of bounds parameter set at version {bmf_version}"
            );
        }
    }

    /// Every v0 fixture parses identically through the node and through the leaf.
    #[test]
    fn adapter_json_v0_through_the_node() {
        for suffix in V0_FIXTURES {
            let file_path = fixture_path(suffix);
            assert_eq!(
                convert_file_path::<AdapterJson>(&file_path),
                convert_file_path::<AdapterJsonV0>(&file_path),
            );
        }
    }

    #[test]
    fn adapter_json_v1_latency() {
        let results = convert_json_v1("v1_latency");
        test_json_v1::validate_adapter_json_v1_latency(&results);
    }

    #[test]
    fn adapter_json_v1_parameters() {
        let results = convert_json_v1("v1_parameters");
        test_json_v1::validate_adapter_json_v1_parameters(&results);
    }

    #[test]
    fn adapter_json_v1_named() {
        let results = convert_json_v1("v1_named");
        test_json_v1::validate_adapter_json_v1_named(&results);
    }

    #[test]
    fn adapter_json_v1_cap() {
        let results = convert_json_v1("v1_cap");
        test_json_v1::validate_adapter_json_v1_cap(&results);
    }

    #[test]
    fn adapter_json_v1_canonical() {
        let results = convert_json_v1("v1_canonical");
        test_json_v1::validate_adapter_json_v1_canonical(&results);
    }

    /// Every v1 fixture parses identically through the node at version 1 and
    /// through the leaf.
    #[test]
    fn adapter_json_v1_through_the_node() {
        for suffix in V1_FIXTURES {
            assert_eq!(
                convert_json_v1(suffix),
                convert_file_path::<AdapterJsonV1>(&fixture_path(suffix)),
                "{suffix}"
            );
        }
    }

    /// A payload that mixes an object shaped benchmark and an array shaped one
    /// fails both leaves and therefore the node at either version. This is the
    /// all or nothing rule.
    #[test]
    fn adapter_json_mixed_versions_fails() {
        let file_path = fixture_path("mixed_versions");
        assert!(
            opt_convert_file_path::<AdapterJsonV0>(&file_path, Settings::default()).is_none(),
            "expected a mixed version payload to fail json_v0"
        );
        assert!(
            opt_convert_file_path::<AdapterJsonV1>(&file_path, Settings::default()).is_none(),
            "expected a mixed version payload to fail json_v1"
        );
        for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
            assert!(
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(bmf_version))
                    .is_none(),
                "expected a mixed version payload to fail the json node at version {bmf_version}"
            );
        }
    }

    /// An empty payload parses as whichever version it declares.
    #[test]
    fn adapter_json_empty_parses_as_the_declared_version() {
        for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
            let results = AdapterJson::parse("{}", version_settings(bmf_version)).unwrap();
            assert!(results.is_empty());
            assert_eq!(results.version, bmf_version);
        }
    }

    /// An absent `bmf_version` is version 0, so the two parse to the same bytes.
    #[test]
    fn adapter_json_absent_version_is_version_0() {
        for suffix in JSON_FIXTURES {
            let file_path = fixture_path(suffix);
            assert_eq!(
                opt_convert_file_path::<AdapterJson>(&file_path, Settings::default()),
                opt_convert_file_path::<AdapterJson>(&file_path, version_settings(BmfVersion::V0)),
                "{suffix}"
            );
        }
    }

    /// Version 1 parses every v1 fixture and refuses every other one.
    #[test]
    fn adapter_json_version_1_parses_v1_only() {
        let settings = version_settings(BmfVersion::V1);
        for suffix in V1_FIXTURES {
            let file_path = fixture_path(suffix);
            assert_eq!(
                opt_convert_file_path::<AdapterJson>(&file_path, settings),
                Some(convert_file_path::<AdapterJsonV1>(&file_path)),
                "{suffix}"
            );
        }
        for suffix in V0_FIXTURES.into_iter().chain(UNCLAIMED_FIXTURES) {
            assert!(
                opt_convert_file_path::<AdapterJson>(&fixture_path(suffix), settings).is_none(),
                "expected the json node to refuse {suffix} at version 1"
            );
        }
    }

    /// Version 0 parses every v0 fixture and refuses every other one.
    #[test]
    fn adapter_json_version_0_parses_v0_only() {
        let settings = version_settings(BmfVersion::V0);
        for suffix in V0_FIXTURES {
            let file_path = fixture_path(suffix);
            assert_eq!(
                opt_convert_file_path::<AdapterJson>(&file_path, settings),
                Some(convert_file_path::<AdapterJsonV0>(&file_path)),
                "{suffix}"
            );
        }
        for suffix in V1_FIXTURES.into_iter().chain(UNCLAIMED_FIXTURES) {
            assert!(
                opt_convert_file_path::<AdapterJson>(&fixture_path(suffix), settings).is_none(),
                "expected the json node to refuse {suffix} at version 0"
            );
        }
    }

    /// A leaf is an exact statement of one shape, so the declared version does
    /// not change what it parses.
    #[test]
    fn adapter_json_leaves_ignore_the_version() {
        for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
            let settings = version_settings(bmf_version);
            for suffix in V1_FIXTURES {
                let file_path = fixture_path(suffix);
                assert!(
                    opt_convert_file_path::<AdapterJsonV0>(&file_path, settings).is_none(),
                    "expected the json_v0 leaf to reject the v1 payload {suffix} at version {bmf_version}"
                );
                assert_eq!(
                    opt_convert_file_path::<AdapterJsonV1>(&file_path, settings),
                    Some(convert_file_path::<AdapterJsonV1>(&file_path)),
                    "{suffix} at version {bmf_version}"
                );
            }
            for suffix in V0_FIXTURES {
                let file_path = fixture_path(suffix);
                assert!(
                    opt_convert_file_path::<AdapterJsonV1>(&file_path, settings).is_none(),
                    "expected the json_v1 leaf to reject the v0 payload {suffix} at version {bmf_version}"
                );
                assert_eq!(
                    opt_convert_file_path::<AdapterJsonV0>(&file_path, settings),
                    Some(convert_file_path::<AdapterJsonV0>(&file_path)),
                    "{suffix} at version {bmf_version}"
                );
            }
        }
    }
}
