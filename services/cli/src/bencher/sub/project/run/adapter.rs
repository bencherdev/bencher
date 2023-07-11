use bencher_client::types::JsonAdapter;

use crate::cli::project::run::CliRunAdapter;

#[derive(Debug, Clone, Copy)]
pub enum RunAdapter {
    Magic,
    Json,
    CSharp,
    CSharpDotNet,
    Cpp,
    CppCatch2,
    CppGoogle,
    Go,
    GoBench,
    Java,
    JavaJmh,
    Js,
    JsBenchmark,
    JsTime,
    Python,
    PythonAsv,
    PythonPytest,
    Ruby,
    RubyBenchmark,
    Rust,
    RustBench,
    RustCriterion,
    RustIai,
}

impl From<CliRunAdapter> for RunAdapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Magic => Self::Magic,
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::CSharp => Self::CSharp,
            CliRunAdapter::CSharpDotNet => Self::CSharpDotNet,
            CliRunAdapter::Cpp => Self::Cpp,
            CliRunAdapter::CppCatch2 => Self::CppCatch2,
            CliRunAdapter::CppGoogle => Self::CppGoogle,
            CliRunAdapter::Go => Self::Go,
            CliRunAdapter::GoBench => Self::GoBench,
            CliRunAdapter::Java => Self::Java,
            CliRunAdapter::JavaJmh => Self::JavaJmh,
            CliRunAdapter::Js => Self::Js,
            CliRunAdapter::JsBenchmark => Self::JsBenchmark,
            CliRunAdapter::JsTime => Self::JsTime,
            CliRunAdapter::Python => Self::Python,
            CliRunAdapter::PythonAsv => Self::PythonAsv,
            CliRunAdapter::PythonPytest => Self::PythonPytest,
            CliRunAdapter::Ruby => Self::Ruby,
            CliRunAdapter::RubyBenchmark => Self::RubyBenchmark,
            CliRunAdapter::Rust => Self::Rust,
            CliRunAdapter::RustBench => Self::RustBench,
            CliRunAdapter::RustCriterion => Self::RustCriterion,
            CliRunAdapter::RustIai => Self::RustIai,
        }
    }
}

impl From<RunAdapter> for JsonAdapter {
    fn from(adapter: RunAdapter) -> Self {
        match adapter {
            RunAdapter::Magic => Self::Magic,
            RunAdapter::Json => Self::Json,
            RunAdapter::CSharp => Self::CSharp,
            RunAdapter::CSharpDotNet => Self::CSharpDotNet,
            RunAdapter::Cpp => Self::Cpp,
            RunAdapter::CppCatch2 => Self::CppCatch2,
            RunAdapter::CppGoogle => Self::CppGoogle,
            RunAdapter::Go => Self::Go,
            RunAdapter::GoBench => Self::GoBench,
            RunAdapter::Java => Self::Java,
            RunAdapter::JavaJmh => Self::JavaJmh,
            RunAdapter::Js => Self::Js,
            RunAdapter::JsBenchmark => Self::JsBenchmark,
            RunAdapter::JsTime => Self::JsTime,
            RunAdapter::Python => Self::Python,
            RunAdapter::PythonAsv => Self::PythonAsv,
            RunAdapter::PythonPytest => Self::PythonPytest,
            RunAdapter::Ruby => Self::Ruby,
            RunAdapter::RubyBenchmark => Self::RubyBenchmark,
            RunAdapter::Rust => Self::Rust,
            RunAdapter::RustBench => Self::RustBench,
            RunAdapter::RustCriterion => Self::RustCriterion,
            RunAdapter::RustIai => Self::RustIai,
        }
    }
}
