use bencher_client::types::Adapter;

use crate::parser::project::run::CliRunAdapter;

impl From<CliRunAdapter> for Adapter {
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
