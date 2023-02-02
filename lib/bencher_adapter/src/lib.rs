pub mod adapters;
pub mod error;
pub mod results;

use adapters::{
    c_sharp::{dot_net::AdapterCSharpDotNet, AdapterCSharp},
    cpp::AdapterCpp,
    cpp::{catch2::AdapterCppCatch2, google::AdapterCppGoogle},
    go::bench::AdapterGoBench,
    go::AdapterGo,
    java::{jmh::AdapterJavaJmh, AdapterJava},
    json::AdapterJson,
    magic::AdapterMagic,
    rust::AdapterRust,
    rust::{bench::AdapterRustBench, criterion::AdapterRustCriterion},
};
use bencher_json::project::report::JsonAdapter;
pub use error::AdapterError;
pub use results::{adapter_results::AdapterResults, AdapterResultsArray};

pub trait Adapter {
    fn convert(&self, input: &str) -> Option<AdapterResults> {
        Self::parse(input)
    }

    fn parse(input: &str) -> Option<AdapterResults>;
}

impl Adapter for JsonAdapter {
    fn convert(&self, input: &str) -> Option<AdapterResults> {
        match self {
            JsonAdapter::Magic => AdapterMagic::parse(input),
            JsonAdapter::Json => AdapterJson::parse(input),
            JsonAdapter::CSharp => AdapterCSharp::parse(input),
            JsonAdapter::CSharpDotNet => AdapterCSharpDotNet::parse(input),
            JsonAdapter::Cpp => AdapterCpp::parse(input),
            JsonAdapter::CppCatch2 => AdapterCppCatch2::parse(input),
            JsonAdapter::CppGoogle => AdapterCppGoogle::parse(input),
            JsonAdapter::Go => AdapterGo::parse(input),
            JsonAdapter::GoBench => AdapterGoBench::parse(input),
            JsonAdapter::Java => AdapterJava::parse(input),
            JsonAdapter::JavaJmh => AdapterJavaJmh::parse(input),
            JsonAdapter::Rust => AdapterRust::parse(input),
            JsonAdapter::RustBench => AdapterRustBench::parse(input),
            JsonAdapter::RustCriterion => AdapterRustCriterion::parse(input),
        }
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterMagic::parse(input)
    }
}
