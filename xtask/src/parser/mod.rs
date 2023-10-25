use clap::{Parser, Subcommand};

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
pub struct CliTranslate {}

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
    pub changelog: Option<String>,

    /// File output path
    #[clap(long)]
    pub path: Option<String>,
}
