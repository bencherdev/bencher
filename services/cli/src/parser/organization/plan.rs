#![cfg(feature = "plus")]

use bencher_json::{Entitlements, NonEmpty, OrganizationResourceId, OrganizationUuid};
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliOrganizationPlan {
    /// Create a subscription plan
    #[clap(alias = "add")]
    Create(CliPlanCreate),
    /// View a subscription plan
    #[clap(alias = "get")]
    View(CliPlanView),
    /// Update a subscription plan
    #[clap(alias = "edit")]
    Update(CliPlanUpdate),
    /// Delete a subscription plan
    #[clap(alias = "rm")]
    Delete(CliPlanDelete),
}

#[derive(Parser, Debug)]
pub struct CliPlanCreate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Checkout session ID (subscription ID when `--skip-remote` is used)
    #[clap(long)]
    pub checkout: NonEmpty,

    /// Plan level
    #[clap(value_enum, long)]
    pub level: CliPlanLevel,

    /// License plan entitlements
    #[clap(long)]
    pub entitlements: Option<Entitlements>,

    /// Self-Hosted Organization UUID for license
    #[clap(long, requires = "entitlements")]
    pub self_hosted: Option<OrganizationUuid>,

    /// Skip sending to remote provider
    #[clap(long)]
    pub skip_remote: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanView {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
#[clap(group(
    ArgGroup::new("cancellation")
        .multiple(false)
        .required(true)
        .args(&["cancel", "resume"]),
))]
pub struct CliPlanUpdate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Schedule the metered subscription to cancel at the end of the current billing period
    #[clap(long)]
    pub cancel: bool,

    /// Resume a metered subscription scheduled to cancel at the end of the current billing period
    #[clap(long)]
    pub resume: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanDelete {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Skip sending to remote provider
    #[clap(long)]
    pub skip_remote: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliPlanLevel {
    /// Free
    Free,
    /// Pro
    Pro,
    /// Team
    Team,
    /// Enterprise
    Enterprise,
}
