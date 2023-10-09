use bencher_json::{AlertUuid, ResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliAlert {
    /// List alerts
    #[clap(alias = "ls")]
    List(CliAlertList),
    /// View an alert
    #[clap(alias = "cat")]
    View(CliAlertView),
    // Update an alert
    #[clap(alias = "edit")]
    Update(CliAlertUpdate),
    /// View alert stats
    Stats(CliAlertStats),
}

#[derive(Parser, Debug)]
pub struct CliAlertList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub pagination: CliPagination<CliAlertsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliAlertsSort {
    /// Creation date time of the alert
    Created,
    // Modification date time of the alert
    Modified,
}

#[derive(Parser, Debug)]
pub struct CliAlertView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Alert UUID
    pub alert: AlertUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAlertUpdate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Alert UUID
    pub alert: AlertUuid,

    /// Alert status
    #[clap(long)]
    pub status: Option<CliAlertStatus>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Supported Fold Operations
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliAlertStatus {
    /// Active
    Active,
    /// Dismissed
    Dismissed,
}

#[derive(Parser, Debug)]
pub struct CliAlertStats {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
