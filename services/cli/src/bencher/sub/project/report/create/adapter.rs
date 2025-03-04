use bencher_client::types::Adapter;

use crate::parser::project::report::CliReportAdapter;

impl From<CliReportAdapter> for Adapter {
    fn from(adapter: CliReportAdapter) -> Self {
        match adapter {
            CliReportAdapter::Magic => Self::Magic,
            CliReportAdapter::Json => Self::Json,
            CliReportAdapter::CSharp => Self::CSharp,
            CliReportAdapter::CSharpDotNet => Self::CSharpDotNet,
            CliReportAdapter::Cpp => Self::Cpp,
            CliReportAdapter::CppCatch2 => Self::CppCatch2,
            CliReportAdapter::CppGoogle => Self::CppGoogle,
            CliReportAdapter::Go => Self::Go,
            CliReportAdapter::GoBench => Self::GoBench,
            CliReportAdapter::Java => Self::Java,
            CliReportAdapter::JavaJmh => Self::JavaJmh,
            CliReportAdapter::Js => Self::Js,
            CliReportAdapter::JsBenchmark => Self::JsBenchmark,
            CliReportAdapter::JsTime => Self::JsTime,
            CliReportAdapter::Python => Self::Python,
            CliReportAdapter::PythonAsv => Self::PythonAsv,
            CliReportAdapter::PythonPytest => Self::PythonPytest,
            CliReportAdapter::Ruby => Self::Ruby,
            CliReportAdapter::RubyBenchmark => Self::RubyBenchmark,
            CliReportAdapter::Rust => Self::Rust,
            CliReportAdapter::RustBench => Self::RustBench,
            CliReportAdapter::RustCriterion => Self::RustCriterion,
            CliReportAdapter::RustIai => Self::RustIai,
            CliReportAdapter::RustIaiCallgrind => Self::RustIaiCallgrind,
            CliReportAdapter::Shell => Self::Shell,
            CliReportAdapter::ShellHyperfine => Self::ShellHyperfine,
        }
    }
}
