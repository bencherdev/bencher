use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

/// Bencher CLI
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct CliTask {
    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: CliSub,
}

#[allow(variant_size_differences)]
#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Generate typeshare
    Typeshare(CliTypeshare),
    /// Generate OpenAPI spec
    Swagger(CliSwagger),
    /// Generate typeshare and OpenAPI spec
    Types(CliTypes),
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
    /// Generate release notes
    ReleaseNotes(CliReleaseNotes),
}

#[derive(Parser, Debug)]
pub struct CliTypeshare {}

#[derive(Parser, Debug)]
pub struct CliSwagger {}

#[derive(Parser, Debug)]
pub struct CliTypes {}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliStats {
    /// Stats JSON
    pub stats: String,
}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliPrompt {
    /// Text prompt
    pub prompt: String,
}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliTranslate {
    /// File input path
    pub input_path: Utf8PathBuf,

    // Target language
    #[clap(value_enum, long)]
    pub lang: Option<Vec<CliLanguage>>,

    /// File output path
    #[clap(long)]
    pub output_path: Option<Utf8PathBuf>,
}

#[cfg(feature = "plus")]
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliLanguage {
    #[clap(alias = "de")]
    German,
    #[clap(alias = "es")]
    Spanish,
    #[clap(alias = "fr")]
    French,
    #[clap(alias = "ja")]
    Japanese,
    #[clap(alias = "ko")]
    Korean,
    #[clap(alias = "pt")]
    Portuguese,
    #[clap(alias = "ru")]
    Russian,
    #[clap(alias = "zh")]
    Chinese,
}

#[derive(Parser, Debug)]
pub struct CliFlyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}

#[derive(Parser, Debug)]
pub struct CliNetlifyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}

#[derive(Parser, Debug)]
pub struct CliReleaseNotes {
    /// Changelog path
    #[clap(long)]
    pub changelog: Option<Utf8PathBuf>,

    /// File output path
    #[clap(long)]
    pub path: Option<Utf8PathBuf>,
}
