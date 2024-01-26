use clap::{Parser, Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum TaskIndex {
    /// Generate a license key
    Update(TaskIndexUpdate),
    /// Validate a license key
    Delete(TaskIndexDelete),
}

#[derive(Parser, Debug)]
pub struct TaskIndexUpdate {
    /// Search engine
    #[clap(value_enum, long)]
    pub engine: Option<TaskSearchEngine>,

    /// URL
    pub url: Vec<url::Url>,
}

pub type TaskIndexDelete = TaskIndexUpdate;

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskSearchEngine {
    /// Google
    Google,
}
