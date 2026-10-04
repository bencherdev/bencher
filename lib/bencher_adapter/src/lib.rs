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
    shell::{AdapterShell, hyperfine::AdapterShellHyperfine},
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

use crate::adapters::rust::gungraun::AdapterRustGungraun;

pub trait Adaptable {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        Self::parse(input, settings)
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults>;
}

impl Adaptable for Adapter {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        match self {
            Self::Magic => AdapterMagic::parse(input, settings),
            Self::Json => AdapterJson::parse(input, settings),
            Self::JsonV0 => AdapterJsonV0::parse(input, settings),
            Self::JsonV1 => AdapterJsonV1::parse(input, settings),
            Self::CSharp => AdapterCSharp::parse(input, settings),
            Self::CSharpDotNet => AdapterCSharpDotNet::parse(input, settings),
            Self::Cpp => AdapterCpp::parse(input, settings),
            Self::CppCatch2 => AdapterCppCatch2::parse(input, settings),
            Self::CppGoogle => AdapterCppGoogle::parse(input, settings),
            Self::Dart => AdapterDart::parse(input, settings),
            Self::DartBenchmarkHarness => AdapterDartBenchmarkHarness::parse(input, settings),
            Self::Go => AdapterGo::parse(input, settings),
            Self::GoBench => AdapterGoBench::parse(input, settings),
            Self::Java => AdapterJava::parse(input, settings),
            Self::JavaJmh => AdapterJavaJmh::parse(input, settings),
            Self::Js => AdapterJs::parse(input, settings),
            Self::JsBenchmark => AdapterJsBenchmark::parse(input, settings),
            Self::JsTime => AdapterJsTime::parse(input, settings),
            Self::JsVitest => AdapterJsVitest::parse(input, settings),
            Self::Python => AdapterPython::parse(input, settings),
            Self::PythonAsv => AdapterPythonAsv::parse(input, settings),
            Self::PythonPytest => AdapterPythonPytest::parse(input, settings),
            Self::Ruby => AdapterRuby::parse(input, settings),
            Self::RubyBenchmark => AdapterRubyBenchmark::parse(input, settings),
            Self::Rust => AdapterRust::parse(input, settings),
            Self::RustBench => AdapterRustBench::parse(input, settings),
            Self::RustCriterion => AdapterRustCriterion::parse(input, settings),
            Self::RustIai => AdapterRustIai::parse(input, settings),
            Self::RustGungraun => AdapterRustGungraun::parse(input, settings),
            Self::RustGungraunStdout => AdapterRustGungraunStdout::parse(input, settings),
            Self::RustGungraunJson => AdapterRustGungraunJson::parse(input, settings),
            Self::Shell => AdapterShell::parse(input, settings),
            Self::ShellHyperfine => AdapterShellHyperfine::parse(input, settings),
        }
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterMagic::parse(input, settings)
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
