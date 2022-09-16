use bencher_json::ResourceId;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliAlert {
    /// List alerts
    #[clap(alias = "ls")]
    List(CliAlertList),
    /// View a alert
    View(CliAlertView),
}

#[derive(Parser, Debug)]
pub struct CliAlertList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAlertView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Alert UUID
    pub alert: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
