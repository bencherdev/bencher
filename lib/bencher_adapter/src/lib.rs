pub mod adapters;
pub mod error;
pub mod results;

use adapters::{
    c_sharp::{dot_net::AdapterCSharpDotNet, AdapterCSharp},
    cpp::{catch2::AdapterCppCatch2, google::AdapterCppGoogle, AdapterCpp},
    go::{bench::AdapterGoBench, AdapterGo},
    java::{jmh::AdapterJavaJmh, AdapterJava},
    js::{benchmark::AdapterJsBenchmark, time::AdapterJsTime, AdapterJs},
    json::AdapterJson,
    magic::AdapterMagic,
    python::{asv::AdapterPythonAsv, pytest::AdapterPythonPytest, AdapterPython},
    ruby::{benchmark::AdapterRubyBenchmark, AdapterRuby},
    rust::{
        bench::AdapterRustBench, criterion::AdapterRustCriterion, iai::AdapterRustIai, AdapterRust,
    },
    shell::{hyperfine::AdapterShellHyperfine, AdapterShell},
};
use bencher_json::project::report::{Adapter, JsonAverage};
pub use bencher_json::{BenchmarkName, JsonMetric};
pub use error::AdapterError;
pub use results::{adapter_results::AdapterResults, AdapterResultsArray};

pub trait Adaptable {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        Self::parse(input, settings)
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults>;
}

impl Adaptable for Adapter {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        match self {
            Adapter::Magic => AdapterMagic::parse(input, settings),
            Adapter::Json => AdapterJson::parse(input, settings),
            Adapter::CSharp => AdapterCSharp::parse(input, settings),
            Adapter::CSharpDotNet => AdapterCSharpDotNet::parse(input, settings),
            Adapter::Cpp => AdapterCpp::parse(input, settings),
            Adapter::CppCatch2 => AdapterCppCatch2::parse(input, settings),
            Adapter::CppGoogle => AdapterCppGoogle::parse(input, settings),
            Adapter::Go => AdapterGo::parse(input, settings),
            Adapter::GoBench => AdapterGoBench::parse(input, settings),
            Adapter::Java => AdapterJava::parse(input, settings),
            Adapter::JavaJmh => AdapterJavaJmh::parse(input, settings),
            Adapter::Js => AdapterJs::parse(input, settings),
            Adapter::JsBenchmark => AdapterJsBenchmark::parse(input, settings),
            Adapter::JsTime => AdapterJsTime::parse(input, settings),
            Adapter::Python => AdapterPython::parse(input, settings),
            Adapter::PythonAsv => AdapterPythonAsv::parse(input, settings),
            Adapter::PythonPytest => AdapterPythonPytest::parse(input, settings),
            Adapter::Ruby => AdapterRuby::parse(input, settings),
            Adapter::RubyBenchmark => AdapterRubyBenchmark::parse(input, settings),
            Adapter::Rust => AdapterRust::parse(input, settings),
            Adapter::RustBench => AdapterRustBench::parse(input, settings),
            Adapter::RustCriterion => AdapterRustCriterion::parse(input, settings),
            Adapter::RustIai => AdapterRustIai::parse(input, settings),
            Adapter::Shell => AdapterShell::parse(input, settings),
            Adapter::ShellHyperfine => AdapterShellHyperfine::parse(input, settings),
        }
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterMagic::parse(input, settings)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Settings {
    pub average: Option<JsonAverage>,
}

impl Settings {
    pub fn new(average: Option<JsonAverage>) -> Self {
        Self { average }
    }
}
