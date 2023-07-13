#![cfg(feature = "plus")]

use bencher_json::{CardCvc, CardNumber, ExpirationMonth, ExpirationYear, ResourceId};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliOrganizationPlan {
    /// Create a metered subscription plan
    #[clap(alias = "add")]
    Create(CliPlanCreate),
    /// View a metered subscription plan
    View(CliPlanView),
}

#[derive(Parser, Debug)]
pub struct CliPlanCreate {
    /// Organization slug or UUID
    pub org: ResourceId,

    #[clap(flatten)]
    pub card: CliPlanCard,

    /// Plan level
    #[clap(value_enum, long)]
    pub level: CliPlanLevel,

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
