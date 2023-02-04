use bencher_json::project::report::JsonAdapter;

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
    Rust,
    RustBench,
    RustCriterion,
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
            CliRunAdapter::Rust => Self::Rust,
            CliRunAdapter::RustBench => Self::RustBench,
            CliRunAdapter::RustCriterion => Self::RustCriterion,
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
            RunAdapter::Rust => Self::Rust,
            RunAdapter::RustBench => Self::RustBench,
            RunAdapter::RustCriterion => Self::RustCriterion,
        }
    }
}
