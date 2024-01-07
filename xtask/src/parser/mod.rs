use clap::{Parser, Subcommand};

mod notify;
mod package;
#[cfg(feature = "plus")]
mod plus;
mod release;
mod template;
mod test;
mod types;

pub use notify::TaskNotify;
#[cfg(target_os = "linux")]
pub use package::TaskDeb;
pub use package::TaskMan;
pub use plus::{
    prompt::{TaskLanguage, TaskPrompt, TaskTranslate},
    stats::TaskStats,
};
pub use release::TaskReleaseNotes;
pub use template::{TaskTemplate, TaskTemplateKind};
pub use test::{TaskNetlifyTest, TaskSmokeTest, TaskTestEnvironment};
pub use types::{TaskSwagger, TaskTypes, TaskTypeshare};

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
    /// Generate typeshare
    Typeshare(TaskTypeshare),
    /// Generate OpenAPI spec
    Swagger(TaskSwagger),
    /// Generate typeshare and OpenAPI spec
    Types(TaskTypes),
    /// Template CLI install scripts
    Template(TaskTemplate),
    #[cfg(feature = "plus")]
    /// Send stats to bencher.dev
    Stats(TaskStats),
    #[cfg(feature = "plus")]
    /// Prompt LLM
    Prompt(TaskPrompt),
    #[cfg(feature = "plus")]
    /// Prompt LLM to translate
    Translate(TaskTranslate),
    /// Run smoke test
    SmokeTest(TaskSmokeTest),
    /// Run tests against Netlify deployment
    NetlifyTest(TaskNetlifyTest),
    /// Create CLI man page
    Man(TaskMan),
    /// Create CLI .deb
    #[cfg(target_os = "linux")]
    Deb(TaskDeb),
    /// Generate release notes
    ReleaseNotes(TaskReleaseNotes),
    /// Notify
    Notify(TaskNotify),
}
