use bencher_json::project::report::Adapter;

use crate::{
    Adaptable as _, Settings,
    adapters::{
        bmf_v1::rust::{
            bench::{AdapterRustBenchJson, AdapterRustBenchText, AdapterRustBenchV1},
            criterion::AdapterRustCriterionV1,
            criterion_json::AdapterRustCriterionJson,
            criterion_text::AdapterRustCriterionText,
            gungraun::AdapterRustGungraunV1,
            gungraun_json::AdapterRustGungraunJsonV1,
            gungraun_stdout::AdapterRustGungraunStdoutV1,
            iai::AdapterRustIaiV1,
        },
        json::v1::AdapterJsonV1,
    },
    convert_v0,
    results::adapter_results::AdapterResults,
};

mod leaf;
mod magic;
pub mod rust;

use magic::AdapterMagicV1;

/// The BMF version 1 adapter tree.
///
/// An adapter it does not hold yet parses with the version 0 tree, whose results
/// the declared version then refuses, exactly as before this tree existed.
pub(crate) fn convert(adapter: Adapter, input: &str, settings: Settings) -> Option<AdapterResults> {
    match adapter {
        Adapter::Magic => AdapterMagicV1::parse(input, settings),
        Adapter::Json | Adapter::JsonV1 => AdapterJsonV1::parse(input, settings),
        Adapter::RustBench => AdapterRustBenchV1::parse(input, settings),
        Adapter::RustBenchJson => AdapterRustBenchJson::parse(input, settings),
        Adapter::RustBenchText => AdapterRustBenchText::parse(input, settings),
        Adapter::RustCriterion => AdapterRustCriterionV1::parse(input, settings),
        Adapter::RustCriterionJson => AdapterRustCriterionJson::parse(input, settings),
        Adapter::RustCriterionText => AdapterRustCriterionText::parse(input, settings),
        Adapter::RustIai => AdapterRustIaiV1::parse(input, settings),
        Adapter::RustGungraun => AdapterRustGungraunV1::parse(input, settings),
        Adapter::RustGungraunJson => AdapterRustGungraunJsonV1::parse(input, settings),
        Adapter::RustGungraunStdout => AdapterRustGungraunStdoutV1::parse(input, settings),
        Adapter::JsonV0
        | Adapter::Rust
        | Adapter::Cpp
        | Adapter::CppGoogle
        | Adapter::CppCatch2
        | Adapter::Go
        | Adapter::GoBench
        | Adapter::Java
        | Adapter::JavaJmh
        | Adapter::CSharp
        | Adapter::CSharpDotNet
        | Adapter::Js
        | Adapter::JsBenchmark
        | Adapter::JsTime
        | Adapter::JsVitest
        | Adapter::Python
        | Adapter::PythonAsv
        | Adapter::PythonPytest
        | Adapter::Ruby
        | Adapter::RubyBenchmark
        | Adapter::Shell
        | Adapter::ShellHyperfine
        | Adapter::ShellHyperfineV1
        | Adapter::ShellHyperfineV2
        | Adapter::Dart
        | Adapter::DartBenchmarkHarness => convert_v0(adapter, input, settings),
    }
}

#[cfg(test)]
mod test_bmf_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use std::path::{Path, PathBuf};

    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::json::{
            test_json::{V0_FIXTURES, V1_FIXTURES, fixture_path, version_settings},
            v1::AdapterJsonV1,
        },
        convert_v0,
    };

    const ADAPTERS: [Adapter; 39] = [
        Adapter::Magic,
        Adapter::Json,
        Adapter::JsonV0,
        Adapter::JsonV1,
        Adapter::Rust,
        Adapter::RustBench,
        Adapter::RustBenchJson,
        Adapter::RustBenchText,
        Adapter::RustCriterion,
        Adapter::RustCriterionJson,
        Adapter::RustCriterionText,
        Adapter::RustIai,
        Adapter::RustGungraun,
        Adapter::RustGungraunStdout,
        Adapter::RustGungraunJson,
        Adapter::Cpp,
        Adapter::CppGoogle,
        Adapter::CppCatch2,
        Adapter::Go,
        Adapter::GoBench,
        Adapter::Java,
        Adapter::JavaJmh,
        Adapter::CSharp,
        Adapter::CSharpDotNet,
        Adapter::Js,
        Adapter::JsBenchmark,
        Adapter::JsTime,
        Adapter::JsVitest,
        Adapter::Python,
        Adapter::PythonAsv,
        Adapter::PythonPytest,
        Adapter::Ruby,
        Adapter::RubyBenchmark,
        Adapter::Shell,
        Adapter::ShellHyperfine,
        Adapter::ShellHyperfineV1,
        Adapter::ShellHyperfineV2,
        Adapter::Dart,
        Adapter::DartBenchmarkHarness,
    ];

    /// Every adapter that parses a v1 payload at version 1 resolves it through
    /// the `json_v1` leaf, and records that leaf.
    #[test]
    fn bmf_v1_json_payload_resolves_to_json_v1() {
        for adapter in [Adapter::Magic, Adapter::Json, Adapter::JsonV1] {
            for suffix in V1_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                let results = adapter
                    .convert(&input, version_settings(BmfVersion::V1))
                    .unwrap_or_else(|| panic!("expected {adapter} to parse {suffix}"));
                assert_eq!(
                    results.resolved_adapter,
                    Some(Adapter::JsonV1),
                    "{adapter} {suffix}"
                );
                assert_eq!(
                    Some(results),
                    AdapterJsonV1::parse(&input, Settings::default()),
                    "{adapter} {suffix}"
                );
            }
        }
    }

    /// Version 0 parses with the version 0 tree and records no leaf, so a v1
    /// payload is refused there and a v0 payload parses as it always has.
    #[test]
    fn bmf_v1_version_0_records_no_leaf() {
        let settings = version_settings(BmfVersion::V0);
        for adapter in [Adapter::Magic, Adapter::Json] {
            for suffix in V1_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                assert_eq!(
                    adapter.convert(&input, settings),
                    None,
                    "{adapter} {suffix}"
                );
            }
            for suffix in V0_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                let results = adapter
                    .convert(&input, settings)
                    .unwrap_or_else(|| panic!("expected {adapter} to parse {suffix}"));
                assert_eq!(results.resolved_adapter, None, "{adapter} {suffix}");
            }
        }
    }

    /// Every fixture through every adapter the version 1 tree does not hold yet
    /// parses at either version exactly as the version 0 tree parses it.
    #[test]
    fn bmf_v1_every_payload_parses_as_the_version_0_tree() {
        let fixtures = fixtures(Path::new("./tool_output"));
        assert!(fixtures.len() > 64, "found {} fixtures", fixtures.len());
        for path in &fixtures {
            let input = read(path);
            for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
                let settings = version_settings(bmf_version);
                for adapter in ADAPTERS {
                    if bmf_version == BmfVersion::V1 && is_rust(adapter) {
                        continue;
                    }
                    assert_eq!(
                        adapter.convert(&input, settings),
                        convert_v0(adapter, &input, settings),
                        "{adapter} at version {bmf_version}: {}",
                        path.display()
                    );
                }
            }
        }
    }

    fn is_rust(adapter: Adapter) -> bool {
        matches!(
            adapter,
            Adapter::RustBench
                | Adapter::RustBenchJson
                | Adapter::RustBenchText
                | Adapter::RustCriterion
                | Adapter::RustCriterionJson
                | Adapter::RustCriterionText
                | Adapter::RustIai
                | Adapter::RustGungraun
                | Adapter::RustGungraunJson
                | Adapter::RustGungraunStdout
        )
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
    }

    fn fixtures(dir: &Path) -> Vec<PathBuf> {
        let mut fixtures = Vec::new();
        for entry in std::fs::read_dir(dir).expect("Failed to read the fixtures") {
            let path = entry.expect("Failed to read a fixture").path();
            if path.is_dir() {
                fixtures.extend(self::fixtures(&path));
            } else {
                fixtures.push(path);
            }
        }
        fixtures
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use std::collections::BTreeMap;

    use bencher_json::{
        BenchmarkName, BenchmarkNameId, BmfVersion, MeasureNameId, ParameterSet, ResourceName,
        project::report::{Adapter, JsonAverage},
    };
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings, results::adapter_results::AdapterResults};

    /// A measure and every metric expected under it.
    pub(crate) type Expected = (&'static str, Vec<(&'static str, f64)>);

    /// The fixture at `./tool_output/<suffix>` through `adapter` at version 1.
    pub(crate) fn convert_v1(adapter: Adapter, suffix: &str) -> AdapterResults {
        let path = format!("./tool_output/{suffix}");
        let input =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
        adapter
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .unwrap_or_else(|| panic!("expected {adapter} to parse {path}"))
    }

    /// The fixture through `adapter` at version 1 parses the same with either average
    /// in its settings as with none.
    pub(crate) fn assert_ignores_average(adapter: Adapter, suffix: &str) {
        let results = convert_v1(adapter, suffix);
        let input = std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .expect("Failed to read a fixture");
        for average in [JsonAverage::Mean, JsonAverage::Median] {
            assert_eq!(
                adapter.convert(&input, Settings::new(Some(average), BmfVersion::V1)),
                Some(results.clone()),
                "{adapter} {average:?} {suffix}"
            );
        }
    }

    /// The results give exactly these measures a unit, each the unit expected.
    pub(crate) fn assert_units(results: &AdapterResults, expected: &[(&str, &str)]) {
        let expected = expected
            .iter()
            .map(|(measure, unit)| {
                (
                    measure
                        .parse::<MeasureNameId>()
                        .expect("Failed to parse measure"),
                    unit.parse::<ResourceName>().expect("Failed to parse unit"),
                )
            })
            .collect();
        assert_eq!(results.units, expected);
    }

    /// How many variants the results hold across every benchmark.
    pub(crate) fn variants(results: &AdapterResults) -> usize {
        results.inner.values().map(BTreeMap::len).sum()
    }

    /// A statistic with its interval, all three in one measure.
    pub(crate) fn typical(
        measure: &'static str,
        lower: f64,
        estimate: f64,
        upper: f64,
    ) -> Expected {
        (
            measure,
            vec![
                ("typical", estimate),
                ("typical_ci_lower", lower),
                ("typical_ci_upper", upper),
            ],
        )
    }

    /// The variant holds exactly the expected measures, each exactly the expected
    /// metric names, each value equal to within floating point rounding.
    pub(crate) fn assert_variant(
        results: &AdapterResults,
        benchmark: &str,
        parameters: &str,
        expected: &[Expected],
    ) {
        let benchmark_name: BenchmarkName = benchmark.parse().expect("Failed to parse benchmark");
        let parameters: ParameterSet = parameters.parse().expect("Failed to parse parameters");
        let metrics = results
            .inner
            .get(&BenchmarkNameId::new_name(benchmark_name))
            .unwrap_or_else(|| panic!("no benchmark {benchmark}"))
            .get(&parameters)
            .unwrap_or_else(|| panic!("no variant {parameters} of {benchmark}"));
        let actual = metrics
            .inner
            .iter()
            .map(|(measure, metric)| {
                (
                    measure.to_string(),
                    metric
                        .inner
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.into_inner()))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected = expected
            .iter()
            .map(|(measure, metrics)| {
                (
                    (*measure).to_owned(),
                    metrics
                        .iter()
                        .map(|(name, value)| ((*name).to_owned(), *value))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(names(&actual), names(&expected), "{benchmark} {parameters}");
        for (measure, metrics) in &expected {
            for (name, value) in metrics {
                let actual = actual[measure][name];
                assert!(
                    (actual - value).abs() <= value.abs() * 1e-9,
                    "{benchmark} {parameters} {measure} {name}: {actual} != {value}"
                );
            }
        }
    }

    fn names(metrics: &BTreeMap<String, BTreeMap<String, f64>>) -> Vec<(String, Vec<String>)> {
        metrics
            .iter()
            .map(|(measure, metrics)| (measure.clone(), metrics.keys().cloned().collect()))
            .collect()
    }
}
