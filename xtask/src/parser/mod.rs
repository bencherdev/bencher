use clap::{Parser, Subcommand};

mod plus;
mod version;

#[cfg(feature = "plus")]
pub use plus::{
    email_list::TaskEmailList,
    index::{TaskIndex, TaskIndexDelete, TaskIndexUpdate, TaskSearchEngine},
    license::{TaskBillingCycle, TaskLicense, TaskLicenseGenerate, TaskLicenseValidate},
    prompt::{TaskImage, TaskLanguage, TaskPrompt, TaskTranslate},
    stats::TaskStats,
};
pub use version::TaskVersion;

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Get current API version
    Version(TaskVersion),
    #[cfg(feature = "plus")]
    #[clap(subcommand)]
    /// `URLindexing`
    Index(TaskIndex),
    #[cfg(feature = "plus")]
    /// Send stats to bencher.dev
    Stats(TaskStats),
    #[cfg(feature = "plus")]
    /// Prompt LLM
    Prompt(TaskPrompt),
    #[cfg(feature = "plus")]
    /// Prompt LLM to translate
    Translate(TaskTranslate),
    #[cfg(feature = "plus")]
    /// Prompt to generate image
    Image(TaskImage),
    #[cfg(feature = "plus")]
    #[clap(subcommand)]
    /// License management
    License(TaskLicense),
    #[cfg(feature = "plus")]
    /// Generate email list
    EmailList(TaskEmailList),
}
