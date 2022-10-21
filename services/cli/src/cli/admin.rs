use clap::{Parser, Subcommand};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliAdmin {
    /// Restart server
    Restart(CliAdminRestart),
}

#[derive(Parser, Debug)]
pub struct CliAdminRestart {
    #[clap(flatten)]
    pub backend: CliBackend,
}
