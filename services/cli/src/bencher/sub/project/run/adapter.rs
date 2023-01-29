use bencher_json::project::report::JsonAdapter;

use crate::cli::project::run::CliRunAdapter;

#[derive(Debug, Clone, Copy)]
pub enum RunAdapter {
    Magic,
    Json,
    Cpp,
    CppGoogle,
    CppCatch2,
    Go,
    GoBench,
    Rust,
    RustBench,
    RustCriterion,
}

impl From<CliRunAdapter> for RunAdapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Magic => Self::Magic,
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::Cpp => Self::Cpp,
            CliRunAdapter::CppGoogle => Self::CppGoogle,
            CliRunAdapter::CppCatch2 => Self::CppCatch2,
            CliRunAdapter::Go => Self::Go,
            CliRunAdapter::GoBench => Self::GoBench,
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
            RunAdapter::Cpp => Self::Cpp,
            RunAdapter::CppGoogle => Self::CppGoogle,
            RunAdapter::CppCatch2 => Self::CppCatch2,
            RunAdapter::Go => Self::Go,
            RunAdapter::GoBench => Self::GoBench,
            RunAdapter::Rust => Self::Rust,
            RunAdapter::RustBench => Self::RustBench,
            RunAdapter::RustCriterion => Self::RustCriterion,
        }
    }
}
