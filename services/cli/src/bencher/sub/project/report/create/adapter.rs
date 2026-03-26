use bencher_client::types::Adapter;

use crate::parser::project::report::CliReportAdapter;

impl From<CliReportAdapter> for Adapter {
    fn from(adapter: CliReportAdapter) -> Self {
        match adapter {
            CliReportAdapter::Magic => Self::Magic,
            CliReportAdapter::Json => Self::Json,
            CliReportAdapter::CSharpDotNet => Self::CSharpDotNet,
            CliReportAdapter::CppCatch2 => Self::CppCatch2,
            CliReportAdapter::CppGoogle => Self::CppGoogle,
            CliReportAdapter::DartBenchmarkHarness => Self::DartBenchmarkHarness,
            CliReportAdapter::GoBench => Self::GoBench,
            CliReportAdapter::JavaJmh => Self::JavaJmh,
            CliReportAdapter::JsBenchmark => Self::JsBenchmark,
            CliReportAdapter::JsTime => Self::JsTime,
            CliReportAdapter::PythonAsv => Self::PythonAsv,
            CliReportAdapter::PythonPytest => Self::PythonPytest,
            CliReportAdapter::RubyBenchmark => Self::RubyBenchmark,
            CliReportAdapter::RustBench => Self::RustBench,
            CliReportAdapter::RustCriterion => Self::RustCriterion,
            CliReportAdapter::RustIai => Self::RustIai,
            CliReportAdapter::RustGungraun => Self::RustGungraun,
            CliReportAdapter::ShellHyperfine => Self::ShellHyperfine,
        }
    }
}
