#![cfg(feature = "plus")]

use bencher_json::{
    CardCvc, CardNumber, Email, Entitlements, ExpirationMonth, ExpirationYear, NonEmpty,
    OrganizationUuid, ResourceId, UserUuid,
};
use clap::{Args, Parser, Subcommand, ValueEnum};

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

    #[clap(flatten)]
    pub customer: CliPlanCustomer,

    #[clap(flatten)]
    pub card: CliPlanCard,

    /// Plan level
    #[clap(value_enum, long)]
    pub level: CliPlanLevel,

    /// License plan entitlements
    #[clap(long)]
    pub entitlements: Option<Entitlements>,

    /// Self-Hosted Organization UUID for license
    #[clap(long, requires = "entitlements")]
    pub organization: Option<OrganizationUuid>,

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

#[derive(Args, Debug)]
pub struct CliPlanCustomer {
    /// User UUID
    #[clap(long)]
    pub uuid: UserUuid,

    /// Name on card
    #[clap(long)]
    pub name: NonEmpty,

    /// User email
    #[clap(long)]
    pub email: Email,
}

#[derive(Args, Debug)]
pub struct CliPlanCard {
    /// Card number
    #[clap(long)]
    pub number: CardNumber,

    /// Card expiration month
    #[clap(long)]
    pub exp_month: ExpirationMonth,

    /// Card expiration year
    #[clap(long)]
    pub exp_year: ExpirationYear,

    /// Card CVC
    #[clap(long)]
    pub cvc: CardCvc,
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
