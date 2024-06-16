use clap::{Parser, Subcommand};

#[cfg(feature = "admin")]
mod admin;
#[cfg(feature = "plus")]
mod plus;
mod types;
mod version;

#[cfg(feature = "admin")]
pub use admin::email_list::TaskEmailList;
#[cfg(feature = "plus")]
pub use plus::{
    index::{TaskIndex, TaskIndexDelete, TaskIndexUpdate, TaskSearchEngine},
    license::{TaskBillingCycle, TaskLicense, TaskLicenseGenerate, TaskLicenseValidate},
    prompt::{TaskImage, TaskLanguage, TaskPrompt, TaskTranslate},
    stats::TaskStats,
};
pub use types::TaskTypes;
pub use version::TaskVersion;

/// Bencher CLI
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct TaskTask {
    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Get current API version
    Version(TaskVersion),
    /// Generate typeshare and OpenAPI spec
    Types(TaskTypes),
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
    #[cfg(feature = "admin")]
    /// Generate email list
    EmailList(TaskEmailList),
}
