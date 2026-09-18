use clap::{Parser, Subcommand};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliServer {
    /// Server version
    Version(CliVersion),
    /// Server `OpenAPI` Spec
    Spec(CliOpenApiSpec),
    /// Manager server config
    #[clap(subcommand)]
    Config(CliConfig),
    #[cfg(feature = "plus")]
    /// Server usage statistics
    Stats(CliServerStats),
}

#[derive(Parser, Debug)]
pub struct CliVersion {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOpenApiSpec {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Subcommand, Debug)]
pub enum CliConfig {
    /// View server config
    View(CliConfigView),
    /// View console config
    Console(CliConfigConsole),
}

#[derive(Parser, Debug)]
pub struct CliConfigView {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliConfigConsole {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliServerStats {
    #[clap(flatten)]
    pub backend: CliBackend,
}
