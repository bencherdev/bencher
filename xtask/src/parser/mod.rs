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
    Typeshare(CliTypeshare),
    Swagger(CliSwagger),
    Translate(CliTranslate),
    FlyTest(CliFlyTest),
    NetlifyTest(CliNetlifyTest),
    ReleaseNotes(CliReleaseNotes),
}

#[derive(Parser, Debug)]
pub struct CliTypeshare {}

#[derive(Parser, Debug)]
pub struct CliSwagger {}

#[derive(Parser, Debug)]
pub struct CliTranslate {
    // Target language
    #[clap(value_enum, long)]
    pub lang: CliLanguage,

    /// File input path
    #[clap(long)]
    pub input_path: Utf8PathBuf,

    /// File output path
    #[clap(long)]
    pub output_path: Option<Utf8PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliLanguage {
    Arabic,
    Chinese,
    Spanish,
    French,
    German,
    Japanese,
    Kannada,
    Portuguese,
    Russian,
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
