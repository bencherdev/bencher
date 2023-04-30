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
    rust::{bench::AdapterRustBench, criterion::AdapterRustCriterion, AdapterRust},
};
use bencher_json::project::report::{JsonAdapter, JsonAverage};
pub use bencher_json::{BenchmarkName, JsonMetric};
pub use error::AdapterError;
pub use results::{adapter_results::AdapterResults, AdapterResultsArray};

pub trait Adapter {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        Self::parse(input, settings)
    }

    fn parse(input: &str, settings: Settings) -> Option<AdapterResults>;
}

impl Adapter for JsonAdapter {
    fn convert(&self, input: &str, settings: Settings) -> Option<AdapterResults> {
        match self {
            JsonAdapter::Magic => AdapterMagic::parse(input, settings),
            JsonAdapter::Json => AdapterJson::parse(input, settings),
            JsonAdapter::CSharp => AdapterCSharp::parse(input, settings),
            JsonAdapter::CSharpDotNet => AdapterCSharpDotNet::parse(input, settings),
            JsonAdapter::Cpp => AdapterCpp::parse(input, settings),
            JsonAdapter::CppCatch2 => AdapterCppCatch2::parse(input, settings),
            JsonAdapter::CppGoogle => AdapterCppGoogle::parse(input, settings),
            JsonAdapter::Go => AdapterGo::parse(input, settings),
            JsonAdapter::GoBench => AdapterGoBench::parse(input, settings),
            JsonAdapter::Java => AdapterJava::parse(input, settings),
            JsonAdapter::JavaJmh => AdapterJavaJmh::parse(input, settings),
            JsonAdapter::Js => AdapterJs::parse(input, settings),
            JsonAdapter::JsBenchmark => AdapterJsBenchmark::parse(input, settings),
            JsonAdapter::JsTime => AdapterJsTime::parse(input, settings),
            JsonAdapter::Python => AdapterPython::parse(input, settings),
            JsonAdapter::PythonAsv => AdapterPythonAsv::parse(input, settings),
            JsonAdapter::PythonPytest => AdapterPythonPytest::parse(input, settings),
            JsonAdapter::Ruby => AdapterRuby::parse(input, settings),
            JsonAdapter::RubyBenchmark => AdapterRubyBenchmark::parse(input, settings),
            JsonAdapter::Rust => AdapterRust::parse(input, settings),
            JsonAdapter::RustBench => AdapterRustBench::parse(input, settings),
            JsonAdapter::RustCriterion => AdapterRustCriterion::parse(input, settings),
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
