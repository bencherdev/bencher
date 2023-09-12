use bencher_json::project::report::JsonAdapter;

use crate::ApiError;

const MAGIC_INT: i32 = 0;
const JSON_INT: i32 = 10;
const RUST_INT: i32 = 20;
const RUST_BENCH_INT: i32 = 21;
const RUST_CRITERION_INT: i32 = 22;
const RUST_IAI_INT: i32 = 23;
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
const PYTHON_INT: i32 = 80;
const PYTHON_ASV_INT: i32 = 81;
const PYTHON_PYTEST_INT: i32 = 82;
const RUBY_INT: i32 = 90;
const RUBY_BENCHMARK_INT: i32 = 91;

#[derive(Debug, Clone, Copy, FromSqlRow, AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
#[repr(i32)]
pub enum Adapter {
    Magic = MAGIC_INT,
    Json = JSON_INT,
    Rust = RUST_INT,
    RustBench = RUST_BENCH_INT,
    RustCriterion = RUST_CRITERION_INT,
    RustIai = RUST_IAI_INT,
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
    Python = PYTHON_INT,
    PythonAsv = PYTHON_ASV_INT,
    PythonPytest = PYTHON_PYTEST_INT,
    Ruby = RUBY_INT,
    RubyBenchmark = RUBY_BENCHMARK_INT,
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
            RUST_IAI_INT => Ok(Self::RustIai),
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
            PYTHON_INT => Ok(Self::Python),
            PYTHON_ASV_INT => Ok(Self::PythonAsv),
            PYTHON_PYTEST_INT => Ok(Self::PythonPytest),
            RUBY_INT => Ok(Self::Ruby),
            RUBY_BENCHMARK_INT => Ok(Self::RubyBenchmark),
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
            JsonAdapter::RustIai => Self::RustIai,
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
            JsonAdapter::Python => Self::Python,
            JsonAdapter::PythonAsv => Self::PythonAsv,
            JsonAdapter::PythonPytest => Self::PythonPytest,
            JsonAdapter::Ruby => Self::Ruby,
            JsonAdapter::RubyBenchmark => Self::RubyBenchmark,
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
            Adapter::RustIai => Self::RustIai,
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
            Adapter::Python => Self::Python,
            Adapter::PythonAsv => Self::PythonAsv,
            Adapter::PythonPytest => Self::PythonPytest,
            Adapter::Ruby => Self::Ruby,
            Adapter::RubyBenchmark => Self::RubyBenchmark,
        }
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Adapter
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Magic => MAGIC_INT.to_sql(out),
            Self::Json => JSON_INT.to_sql(out),
            Self::Rust => RUST_INT.to_sql(out),
            Self::RustBench => RUST_BENCH_INT.to_sql(out),
            Self::RustCriterion => RUST_CRITERION_INT.to_sql(out),
            Self::RustIai => RUST_IAI_INT.to_sql(out),
            Self::Cpp => CPP_INT.to_sql(out),
            Self::CppGoogle => CPP_GOOGLE_INT.to_sql(out),
            Self::CppCatch2 => CPP_CATCH2_INT.to_sql(out),
            Self::Go => GO_INT.to_sql(out),
            Self::GoBench => GO_BENCH_INT.to_sql(out),
            Self::Java => JAVA_INT.to_sql(out),
            Self::JavaJmh => JAVA_JMH_INT.to_sql(out),
            Self::CSharp => C_SHARP_INT.to_sql(out),
            Self::CSharpDotNet => C_SHARP_DOT_NET_INT.to_sql(out),
            Self::Js => JS_INT.to_sql(out),
            Self::JsBenchmark => JS_BENCHMARK_INT.to_sql(out),
            Self::JsTime => JS_TIME_INT.to_sql(out),
            Self::Python => PYTHON_INT.to_sql(out),
            Self::PythonAsv => PYTHON_ASV_INT.to_sql(out),
            Self::PythonPytest => PYTHON_PYTEST_INT.to_sql(out),
            Self::Ruby => RUBY_INT.to_sql(out),
            Self::RubyBenchmark => RUBY_BENCHMARK_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Adapter
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self::try_from(i32::from_sql(bytes)?)?)
    }
}
