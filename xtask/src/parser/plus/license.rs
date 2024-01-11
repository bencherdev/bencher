use bencher_json::{Entitlements, Jwt, OrganizationUuid, PlanLevel};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum TaskLicense {
    /// Generate a license key
    Generate(TaskLicenseGenerate),
    /// Validate a license key
    Validate(TaskLicenseValidate),
}

#[derive(Parser, Debug)]
pub struct TaskLicenseGenerate {
    /// Organization UUID
    pub organization: OrganizationUuid,

    /// License pem
    #[clap(long, env = "BENCHER_LICENSE_PEM")]
    pub pem: String,

    /// Billing cycle
    #[clap(value_enum, long)]
    pub cycle: TaskBillingCycle,

    /// Plan level
    #[clap(value_enum, long)]
    pub level: PlanLevel,

    /// Plan entitlements
    #[clap(long)]
    pub entitlements: Entitlements,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskBillingCycle {
    /// Monthly billing cycle
    Monthly,
    /// Annual billing cycle
    Annual,
}

#[derive(Parser, Debug)]
pub struct TaskLicenseValidate {
    pub license: Jwt,

    /// License pem
    #[clap(long, env = "BENCHER_LICENSE_PEM")]
    pub pem: String,
}
