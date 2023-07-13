use clap::{Parser, Subcommand};

/// Bencher CLI
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct CliTask {
    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: CliSub,
}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    Fmt,
}
