use bencher_json::project::report::JsonAdapter;

use crate::cli::project::run::CliRunAdapter;

#[derive(Debug, Clone, Copy)]
pub enum RunAdapter {
    Magic,
    Json,
    Rust,
}

impl From<CliRunAdapter> for RunAdapter {
    fn from(adapter: CliRunAdapter) -> Self {
        match adapter {
            CliRunAdapter::Magic => Self::Magic,
            CliRunAdapter::Json => Self::Json,
            CliRunAdapter::Rust => Self::Rust,
        }
    }
}

impl From<RunAdapter> for JsonAdapter {
    fn from(adapter: RunAdapter) -> Self {
        match adapter {
            RunAdapter::Magic => Self::Magic,
            RunAdapter::Json => Self::Json,
            RunAdapter::Rust => Self::Rust,
        }
    }
}
