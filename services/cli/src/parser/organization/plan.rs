#![cfg(feature = "plus")]

use bencher_json::{Entitlements, NonEmpty, OrganizationUuid, ResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliOrganizationPlan {
    /// Create a subscription plan
    #[clap(alias = "add")]
    Create(CliPlanCreate),
    /// View a subscription plan
    #[clap(alias = "get")]
    View(CliPlanView),
    /// Delete a subscription plan
    #[clap(alias = "rm")]
    Delete(CliPlanDelete),
}

#[derive(Parser, Debug)]
pub struct CliPlanCreate {
    /// Organization slug or UUID
    pub org: ResourceId,

    /// Customer ID
    #[clap(long)]
    pub customer: NonEmpty,

    /// Payment method ID
    #[clap(long)]
    pub payment_method: NonEmpty,

    /// Plan level
    #[clap(value_enum, long)]
    pub level: CliPlanLevel,

    /// License plan entitlements
    #[clap(long)]
    pub entitlements: Option<Entitlements>,

    /// Self-Hosted Organization UUID for license
    #[clap(long, requires = "entitlements")]
    pub organization: Option<OrganizationUuid>,

    /// I agree to the Bencher Subscription Agreement (https://bencher.dev/legal/subscription)
    #[clap(long, required = true)]
    pub i_agree: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanView {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanDelete {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliPlanLevel {
    /// Free
    Free,
    /// Team
    Team,
    /// Enterprise
    Enterprise,
}
