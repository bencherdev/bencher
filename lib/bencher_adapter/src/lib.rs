pub mod adapters;
pub mod error;
pub mod results;

#[cfg(test)]
use criterion as _;

pub use adapters::json::v0::{JsonV0Measures, JsonV0Results};
use adapters::{
    c_sharp::{AdapterCSharp, dot_net::AdapterCSharpDotNet},
    cpp::{AdapterCpp, catch2::AdapterCppCatch2, google::AdapterCppGoogle},
    dart::{AdapterDart, benchmark_harness::AdapterDartBenchmarkHarness},
    go::{AdapterGo, bench::AdapterGoBench},
    java::{AdapterJava, jmh::AdapterJavaJmh},
    js::{AdapterJs, benchmark::AdapterJsBenchmark, time::AdapterJsTime, vitest::AdapterJsVitest},
    json::{AdapterJson, v0::AdapterJsonV0, v1::AdapterJsonV1},
    magic::AdapterMagic,
    python::{AdapterPython, asv::AdapterPythonAsv, pytest::AdapterPythonPytest},
    ruby::{AdapterRuby, benchmark::AdapterRubyBenchmark},
    rust::{
        AdapterRust, bench::AdapterRustBench, criterion::AdapterRustCriterion,
        gungraun_json::AdapterRustGungraunJson, gungraun_stdout::AdapterRustGungraunStdout,
        iai::AdapterRustIai,
    },
    shell::{
        AdapterShell,
        hyperfine::{
            AdapterShellHyperfine, v1::AdapterShellHyperfineV1, v2::AdapterShellHyperfineV2,
        },
    },
};
pub use bencher_json::{BenchmarkName, JsonNewMetric};
use bencher_json::{
    BmfVersion,
    project::report::{Adapter, JsonAverage},
};
pub use error::AdapterError;
pub use results::{
    AdapterResultsArray,
    adapter_results::AdapterResults,
    foldable::{FoldableResults, FoldableResultsArray},
};

use crate::adapters::{bmf_v1, rust::gungraun::AdapterRustGungraun};

pub trait Adaptable {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        Self::parse(input, settings)
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults>;
}

impl Adaptable for Adapter {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        if settings.bmf_version == BmfVersion::V1 {
            bmf_v1::convert(*self, input, settings)
        } else {
            convert_v0(*self, input, settings)
        }
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        Self::Magic.convert(input, settings)
    }
}

/// The version 0 adapter tree.
pub(crate) fn convert_v0(
    adapter: Adapter,
    input: &str,
    settings: Settings,
) -> Option<AdapterResults> {
    match adapter {
        Adapter::Magic => AdapterMagic::parse(input, settings),
        Adapter::Json => AdapterJson::parse(input, settings),
        Adapter::JsonV0 => AdapterJsonV0::parse(input, settings),
        Adapter::JsonV1 => AdapterJsonV1::parse(input, settings),
        Adapter::CSharp => AdapterCSharp::parse(input, settings),
        Adapter::CSharpDotNet => AdapterCSharpDotNet::parse(input, settings),
        Adapter::Cpp => AdapterCpp::parse(input, settings),
        Adapter::CppCatch2 | Adapter::CppCatch2Text => AdapterCppCatch2::parse(input, settings),
        Adapter::CppGoogle => AdapterCppGoogle::parse(input, settings),
        Adapter::Dart => AdapterDart::parse(input, settings),
        Adapter::DartBenchmarkHarness => AdapterDartBenchmarkHarness::parse(input, settings),
        Adapter::Go => AdapterGo::parse(input, settings),
        Adapter::GoBench | Adapter::GoBenchText => AdapterGoBench::parse(input, settings),
        Adapter::Java => AdapterJava::parse(input, settings),
        Adapter::JavaJmh => AdapterJavaJmh::parse(input, settings),
        Adapter::Js => AdapterJs::parse(input, settings),
        Adapter::JsBenchmark => AdapterJsBenchmark::parse(input, settings),
        Adapter::JsTime => AdapterJsTime::parse(input, settings),
        Adapter::JsVitest | Adapter::JsVitestV1 => AdapterJsVitest::parse(input, settings),
        Adapter::Python => AdapterPython::parse(input, settings),
        Adapter::PythonAsv | Adapter::PythonAsvText => AdapterPythonAsv::parse(input, settings),
        Adapter::PythonPytest => AdapterPythonPytest::parse(input, settings),
        Adapter::Ruby => AdapterRuby::parse(input, settings),
        Adapter::RubyBenchmark => AdapterRubyBenchmark::parse(input, settings),
        Adapter::Rust => AdapterRust::parse(input, settings),
        Adapter::RustBench | Adapter::RustBenchText => AdapterRustBench::parse(input, settings),
        Adapter::RustCriterion | Adapter::RustCriterionText => {
            AdapterRustCriterion::parse(input, settings)
        },
        // The version 0 tree reads no structured Rust, Catch2, or Go output, no Vitest 5 report,
        // and no asv results file.
        Adapter::RustBenchJson
        | Adapter::RustCriterionJson
        | Adapter::CppCatch2Xml
        | Adapter::GoBenchJson
        | Adapter::JsVitestV5
        | Adapter::PythonAsvJson => None,
        Adapter::RustIai => AdapterRustIai::parse(input, settings),
        Adapter::RustGungraun => AdapterRustGungraun::parse(input, settings),
        Adapter::RustGungraunStdout => AdapterRustGungraunStdout::parse(input, settings),
        Adapter::RustGungraunJson => AdapterRustGungraunJson::parse(input, settings),
        Adapter::Shell => AdapterShell::parse(input, settings),
        Adapter::ShellHyperfine => AdapterShellHyperfine::parse(input, settings),
        Adapter::ShellHyperfineV1 => AdapterShellHyperfineV1::parse(input, settings),
        Adapter::ShellHyperfineV2 => AdapterShellHyperfineV2::parse(input, settings),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Settings {
    pub average: Option<JsonAverage>,
    /// The BMF version the report payload is read as: the one it declared, or its
    /// project's default.
    ///
    /// The `json` node parses with that version's leaf only, and the results array
    /// refuses any payload whose parsed version differs.
    pub bmf_version: BmfVersion,
}

impl Settings {
    pub fn new(average: Option<JsonAverage>, bmf_version: BmfVersion) -> Self {
        Self {
            average,
            bmf_version,
        }
    }
}

#[cfg(test)]
mod test_adapter {
    use bencher_json::{BmfVersion, project::report::Adapter};

    use crate::{Adaptable as _, Settings};

    /// Results parsed without naming an adapter are read by `magic` in the tree of
    /// their version.
    ///
    /// Kills: a parse that claims nothing, and one that claims an empty result.
    #[test]
    fn adapter_parse_reads_with_magic_in_the_versions_tree() {
        for (file_path, bmf_version, resolved_adapter) in [
            (
                "./tool_output/json/report_v1_parameters.json",
                BmfVersion::V1,
                Some(Adapter::JsonV1),
            ),
            ("./tool_output/rust/bench/many.txt", BmfVersion::V0, None),
        ] {
            let input = std::fs::read_to_string(file_path).expect("Failed to read test file");
            let results = Adapter::parse(&input, Settings::new(None, bmf_version))
                .unwrap_or_else(|| panic!("expected magic to claim {file_path}"));
            assert_eq!(
                (results.version, results.resolved_adapter),
                (bmf_version, resolved_adapter),
                "{file_path}"
            );
            assert!(results.has_metrics(), "{file_path}");
        }
    }
}
