use clap::{Parser, Subcommand};

mod notify;
mod package;
#[cfg(feature = "plus")]
mod plus;
mod release;
mod template;
mod test;
mod types;

pub use notify::CliNotify;
pub use package::{CliDeb, CliMan};
pub use plus::{
    prompt::{CliLanguage, CliPrompt, CliTranslate},
    stats::CliStats,
};
pub use release::CliReleaseNotes;
pub use template::{CliTemplate, CliTemplateKind};
pub use test::{CliFlyTest, CliNetlifyTest};
pub use types::{CliSwagger, CliTypes, CliTypeshare};

/// Bencher CLI
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct CliTask {
    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: CliSub,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Generate typeshare
    Typeshare(CliTypeshare),
    /// Generate OpenAPI spec
    Swagger(CliSwagger),
    /// Generate typeshare and OpenAPI spec
    Types(CliTypes),
    /// Template CLI install scripts
    Template(CliTemplate),
    #[cfg(feature = "plus")]
    /// Send stats to bencher.dev
    Stats(CliStats),
    #[cfg(feature = "plus")]
    /// Prompt LLM
    Prompt(CliPrompt),
    #[cfg(feature = "plus")]
    /// Prompt LLM to translate
    Translate(CliTranslate),
    /// Run tests against Fly.io deployment
    FlyTest(CliFlyTest),
    /// Run tests against Netlify deployment
    NetlifyTest(CliNetlifyTest),
    /// Create CLI man page
    Man(CliMan),
    /// Create CLI .deb
    Deb(CliDeb),
    /// Generate release notes
    ReleaseNotes(CliReleaseNotes),
    /// Notify
    Notify(CliNotify),
}
