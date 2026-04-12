use clap::{Parser, Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum TaskIndex {
    /// Submit URLs to search engine indexes
    Update(TaskIndexUpdate),
    /// Remove URLs from search engine indexes
    Delete(TaskIndexDelete),
    /// Submit an English page path and all 8 translated variants
    Docs(TaskIndexDocs),
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

#[derive(Parser, Debug)]
pub struct TaskIndexDocs {
    /// Search engine
    #[clap(value_enum, long)]
    pub engine: Option<TaskSearchEngine>,

    /// Path (e.g. /learn/benchmarking/rust/criterion/)
    pub path: Vec<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskSearchEngine {
    /// Bing
    Bing,
    /// Google
    Google,
}
