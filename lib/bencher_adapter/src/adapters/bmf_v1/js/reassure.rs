use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{BuiltInMeasure as _, default::Latency, reassure::Renders},
        report::Adapter,
    },
};
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings, adapters::bmf_v1::leaf::LeafResults,
    results::adapter_results::AdapterResults,
};

/// Reassure's `.reassure/current.perf`: JSON Lines of an optional header, then one entry per test,
/// each named by Jest's test name.
pub struct AdapterJsReassure;

const NANOS_PER_MILLI: f64 = 1e6;

impl Adaptable for AdapterJsReassure {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut lines = input.lines().peekable();
        lines.next_if(|line| serde_json::from_str::<Header>(line).is_ok());
        let entries = lines
            .map(serde_json::from_str::<Entry>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let mut results = LeafResults::default();
        for entry in entries {
            let Ok(name) = entry.name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut variant) = results.new_variant(name, ParameterSet::default()) else {
                continue;
            };
            let latency = Latency::name_id();
            variant.insert(
                latency.clone(),
                "mean",
                entry.mean_duration * NANOS_PER_MILLI,
            );
            variant.insert(latency, "std_dev", entry.stdev_duration * NANOS_PER_MILLI);
            if entry.kind == Kind::Render {
                let renders = Renders::name_id();
                variant.insert(renders.clone(), "mean", entry.mean_count);
                variant.insert(renders, "std_dev", entry.stdev_count);
            }
        }
        results.finish_with_skips(Adapter::JsReassure)
    }
}

/// The first line the `reassure` command writes, which Jest alone does not.
#[derive(Deserialize)]
struct Header {
    #[serde(rename = "metadata")]
    _metadata: IgnoredAny,
}

/// One test's statistics, its durations in milliseconds.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    #[serde(default, rename = "type")]
    kind: Kind,
    mean_duration: f64,
    stdev_duration: f64,
    mean_count: f64,
    stdev_count: f64,
}

/// What a test measured; an entry from before Reassure measured functions has no type.
#[derive(Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[default]
    Render,
    Function,
    #[serde(rename = "async function")]
    AsyncFunction,
}

#[cfg(test)]
mod test_js_reassure {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency, reassure::Renders},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_v1,
        },
        results::adapter_results::AdapterResults,
    };

    const MS: f64 = 1e6;

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results.inner.keys().map(ToString::to_string).collect()
    }

    fn latency(mean: f64, std_dev: f64) -> Expected {
        (
            Latency::SLUG_STR,
            vec![("mean", mean * MS), ("std_dev", std_dev * MS)],
        )
    }

    fn renders(mean: f64, std_dev: f64) -> Expected {
        (
            Renders::SLUG_STR,
            vec![("mean", mean), ("std_dev", std_dev)],
        )
    }

    /// Kills durations left in milliseconds, a mean and standard deviation swapped or misnamed,
    /// render counts read from the durations, render counts written for a function or an async
    /// function, whose count is always 1, and render counts dropped for a render.
    #[test]
    fn adapter_js_reassure_statistics() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/current.perf");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsReassure));
        assert!(results.skipped.is_empty());
        assert_eq!(
            names(&results),
            [
                "List 100",
                "List with scenario",
                "async function",
                "fib function"
            ]
            .map(ToOwned::to_owned)
            .into()
        );
        assert_variant(
            &results,
            "List 100",
            "{}",
            &[
                latency(2.285_412_900_000_426, 0.195_200_931_132_470_7),
                renders(1.0, 0.0),
            ],
        );
        assert_variant(
            &results,
            "List with scenario",
            "{}",
            &[
                latency(1.783_347_000_000_094_4, 0.153_325_341_764_911_42),
                renders(3.0, 0.0),
            ],
        );
        assert_variant(
            &results,
            "fib function",
            "{}",
            &[latency(0.156_516_900_000_042_37, 0.008_786_269_546_521_468)],
        );
        assert_variant(
            &results,
            "async function",
            "{}",
            &[latency(
                0.000_554_999_999_940_264_4,
                0.000_112_785_194_107_610_9,
            )],
        );
    }

    /// Kills the render counts' standard deviation taken from anything but `stdevCount`, here
    /// the sample standard deviation of the counts 3, 1, 2, 3, 1, and 2.
    #[test]
    fn adapter_js_reassure_render_count_spread() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/counts.perf");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "Counter varying clicks",
            "{}",
            &[
                latency(0.302_623_833_333_351_9, 0.101_772_526_741_671_39),
                renders(2.0, 0.894_427_190_999_915_9),
            ],
        );
    }

    /// Jest names a test by its describe chain, and two files may hold a test of one name.
    ///
    /// Kills a second test of a name already read that overwrites the first or is left
    /// uncounted.
    #[test]
    fn adapter_js_reassure_repeated_name() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/names.perf");
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (0, 1)
        );
        assert_eq!(
            names(&results),
            ["Parser json small", "shared name"]
                .map(ToOwned::to_owned)
                .into()
        );
        assert_variant(
            &results,
            "shared name",
            "{}",
            &[latency(0.000_099_999_999_974_897_95, 0.0)],
        );
    }

    /// Jest alone writes no header and appends each run to the file.
    ///
    /// Kills a header required on the first line, and a later run that overwrites the first or is
    /// left uncounted.
    #[test]
    fn adapter_js_reassure_without_header() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/appended.perf");
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (0, 4)
        );
        assert_eq!(results.inner.len(), 4);
        assert_variant(
            &results,
            "List 100",
            "{}",
            &[
                latency(1.823_200_099_999_758_2, 0.489_200_151_823_555),
                renders(1.0, 0.0),
            ],
        );
    }

    /// Reassure 0.9.0 measured renders only and wrote no type.
    ///
    /// Kills an entry with no type refused, or read as a function with no render counts.
    #[test]
    fn adapter_js_reassure_before_type() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/v0_9_0.perf");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "List with scenario",
            "{}",
            &[
                latency(3.515_789_600_000_789_3, 0.118_653_934_441_975_93),
                renders(3.0, 0.0),
            ],
        );
    }

    /// Kills a test name too long for a benchmark name left uncounted, and a file of such tests
    /// refused, so that their count never reaches the report.
    #[test]
    fn adapter_js_reassure_name_too_long() {
        let results = convert_v1(Adapter::JsReassure, "js/reassure/long_only.perf");
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 0)
        );
        assert!(results.inner.is_empty());
    }

    /// A run that Jest stops while Reassure writes leaves a cut last line, which Reassure's own
    /// comparison refuses.
    ///
    /// Kills a line that is not an entry dropped while the others are read.
    #[test]
    fn adapter_js_reassure_refuses_a_cut_line() {
        let input = std::fs::read_to_string("./tool_output/js/reassure/current.perf")
            .expect("Failed to read a fixture");
        let cut = input
            .get(..input.len() - 40)
            .expect("Failed to cut the fixture");
        assert_eq!(
            Adapter::JsReassure.convert(cut, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_reassure_ignores_average() {
        for suffix in [
            "js/reassure/current.perf",
            "js/reassure/appended.perf",
            "js/reassure/v0_9_0.perf",
        ] {
            assert_ignores_average(Adapter::JsReassure, suffix);
        }
    }
}
