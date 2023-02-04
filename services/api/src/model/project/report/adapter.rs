use bencher_json::project::report::JsonAdapter;

use crate::ApiError;

const MAGIC_INT: i32 = 0;
const JSON_INT: i32 = 10;
const RUST_INT: i32 = 20;
const RUST_BENCH_INT: i32 = 21;
const RUST_CRITERION_INT: i32 = 22;
const CPP_INT: i32 = 30;
const CPP_GOOGLE_INT: i32 = 31;
const CPP_CATCH2_INT: i32 = 32;
const GO_INT: i32 = 40;
const GO_BENCH_INT: i32 = 41;
const JAVA_INT: i32 = 50;
const JAVA_JMH_INT: i32 = 51;
const C_SHARP_INT: i32 = 60;
const C_SHARP_DOT_NET_INT: i32 = 61;
const JS_INT: i32 = 70;
const JS_BENCHMARK_INT: i32 = 71;
const JS_TIME_INT: i32 = 72;

#[repr(i32)]
pub enum Adapter {
    Magic = MAGIC_INT,
    Json = JSON_INT,
    Rust = RUST_INT,
    RustBench = RUST_BENCH_INT,
    RustCriterion = RUST_CRITERION_INT,
    Cpp = CPP_INT,
    CppGoogle = CPP_GOOGLE_INT,
    CppCatch2 = CPP_CATCH2_INT,
    Go = GO_INT,
    GoBench = GO_BENCH_INT,
    Java = JAVA_INT,
    JavaJmh = JAVA_JMH_INT,
    CSharp = C_SHARP_INT,
    CSharpDotNet = C_SHARP_DOT_NET_INT,
    Js = JS_INT,
    JsBenchmark = JS_BENCHMARK_INT,
    JsTime = JS_TIME_INT,
}

impl TryFrom<i32> for Adapter {
    type Error = ApiError;

    fn try_from(adapter: i32) -> Result<Self, Self::Error> {
        match adapter {
            MAGIC_INT => Ok(Self::Magic),
            JSON_INT => Ok(Self::Json),
            RUST_INT => Ok(Self::Rust),
            RUST_BENCH_INT => Ok(Self::RustBench),
            RUST_CRITERION_INT => Ok(Self::RustCriterion),
            CPP_INT => Ok(Self::Cpp),
            CPP_GOOGLE_INT => Ok(Self::CppGoogle),
            CPP_CATCH2_INT => Ok(Self::CppCatch2),
            GO_INT => Ok(Self::Go),
            GO_BENCH_INT => Ok(Self::GoBench),
            JAVA_INT => Ok(Self::Java),
            JAVA_JMH_INT => Ok(Self::JavaJmh),
            C_SHARP_INT => Ok(Self::CSharp),
            C_SHARP_DOT_NET_INT => Ok(Self::CSharpDotNet),
            JS_INT => Ok(Self::Js),
            JS_BENCHMARK_INT => Ok(Self::JsBenchmark),
            JS_TIME_INT => Ok(Self::JsTime),
            _ => Err(ApiError::AdapterInt(adapter)),
        }
    }
}

impl From<JsonAdapter> for Adapter {
    fn from(adapter: JsonAdapter) -> Self {
        match adapter {
            JsonAdapter::Magic => Self::Magic,
            JsonAdapter::Json => Self::Json,
            JsonAdapter::Rust => Self::Rust,
            JsonAdapter::RustBench => Self::RustBench,
            JsonAdapter::RustCriterion => Self::RustCriterion,
            JsonAdapter::Cpp => Self::Cpp,
            JsonAdapter::CppGoogle => Self::CppGoogle,
            JsonAdapter::CppCatch2 => Self::CppCatch2,
            JsonAdapter::Go => Self::Go,
            JsonAdapter::GoBench => Self::GoBench,
            JsonAdapter::Java => Self::Java,
            JsonAdapter::JavaJmh => Self::JavaJmh,
            JsonAdapter::CSharp => Self::CSharp,
            JsonAdapter::CSharpDotNet => Self::CSharpDotNet,
            JsonAdapter::Js => Self::Js,
            JsonAdapter::JsBenchmark => Self::JsBenchmark,
            JsonAdapter::JsTime => Self::JsTime,
        }
    }
}

impl From<Adapter> for JsonAdapter {
    fn from(adapter: Adapter) -> Self {
        match adapter {
            Adapter::Magic => Self::Magic,
            Adapter::Json => Self::Json,
            Adapter::Rust => Self::Rust,
            Adapter::RustBench => Self::RustBench,
            Adapter::RustCriterion => Self::RustCriterion,
            Adapter::Cpp => Self::Cpp,
            Adapter::CppGoogle => Self::CppGoogle,
            Adapter::CppCatch2 => Self::CppCatch2,
            Adapter::Go => Self::Go,
            Adapter::GoBench => Self::GoBench,
            Adapter::Java => Self::Java,
            Adapter::JavaJmh => Self::JavaJmh,
            Adapter::CSharp => Self::CSharp,
            Adapter::CSharpDotNet => Self::CSharpDotNet,
            Adapter::Js => Self::Js,
            Adapter::JsBenchmark => Self::JsBenchmark,
            Adapter::JsTime => Self::JsTime,
        }
    }
}
