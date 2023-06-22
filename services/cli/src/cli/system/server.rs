use clap::{Parser, Subcommand, ValueEnum};

use crate::cli::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliServer {
    /// Ping server
    Ping(CliPing),
    /// Server version
    Version(CliVersion),
    /// Restart server
    Restart(CliRestart),
    /// Manager server config
    #[clap(subcommand)]
    Config(CliConfig),
    /// Backup database
    Backup(CliBackup),
}

#[derive(Parser, Debug)]
pub struct CliPing {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliVersion {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliRestart {
    /// Server restart delay seconds (default: 3)
    #[clap(long)]
    pub delay: Option<u64>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Subcommand, Debug)]
pub enum CliConfig {
    /// Update server config and restart
    Update(CliConfigUpdate),
    /// View server config
    View(CliConfigView),
    /// View server endpoint
    Endpoint(CliConfigEndpoint),
}

#[derive(Parser, Debug)]
pub struct CliConfigUpdate {
    /// New server config
    #[clap(long)]
    pub config: String,

    /// Server restart delay seconds (default: 3)
    #[clap(long)]
    pub delay: Option<u64>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliConfigView {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliConfigEndpoint {
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
