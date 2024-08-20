use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliServer {
    /// Server version
    Version(CliVersion),
    /// Server `OpenAPI` Spec
    Spec(CliSpec),
    /// Restart server
    Restart(CliRestart),
    /// Manager server config
    #[clap(subcommand)]
    Config(CliConfig),
    /// Backup database
    Backup(CliBackup),
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
pub struct CliSpec {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliRestart {
    /// Server restart delay seconds
    #[clap(long, default_value = "3")]
    pub delay: u64,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Subcommand, Debug)]
pub enum CliConfig {
    /// View server config
    View(CliConfigView),
    /// Update server config and restart
    Update(CliConfigUpdate),
    /// View console config
    Console(CliConfigConsole),
}

#[derive(Parser, Debug)]
pub struct CliConfigView {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliConfigUpdate {
    /// New server config
    #[clap(long)]
    pub config: String,

    /// Server restart delay seconds
    #[clap(long, default_value = "3")]
    pub delay: u64,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliConfigConsole {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBackup {
    /// Compress database backup with gzip
    #[clap(long)]
    pub compress: bool,

    /// Save database backup to data store
    #[clap(long)]
    pub data_store: Option<CliBackupDataStore>,

    /// Remove backups
    #[clap(long)]
    pub rm: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Supported Fold Operations
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliBackupDataStore {
    /// AWS S3
    AwsS3,
}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliServerStats {
    #[clap(flatten)]
    pub backend: CliBackend,
}
