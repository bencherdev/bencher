use bencher_json::{NonEmpty, ResourceId, Slug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::cli::CliBackend;

pub mod member;
#[cfg(feature = "plus")]
pub mod plan;
#[cfg(feature = "plus")]
pub mod usage;

#[cfg(feature = "plus")]
use self::{plan::CliOrganizationPlan, usage::CliOrganizationUsage};

#[derive(Subcommand, Debug)]
pub enum CliOrganization {
    /// List organizations
    #[clap(alias = "ls")]
    List(CliOrganizationList),
    /// Create a organization
    #[clap(alias = "add")]
    Create(CliOrganizationCreate),
    /// View a organization
    View(CliOrganizationView),
    /// Check organization permission
    Allowed(CliOrganizationAllowed),

    #[cfg(feature = "plus")]
    /// Organization metered subscription plan
    #[clap(subcommand)]
    Plan(CliOrganizationPlan),

    #[cfg(feature = "plus")]
    /// Check organization metrics usage
    Usage(CliOrganizationUsage),
}

#[derive(Parser, Debug)]
pub struct CliOrganizationList {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationCreate {
    /// Organization name
    pub name: NonEmpty,

    /// Organization slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationView {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationAllowed {
    /// Organization permission
    #[clap(long)]
    pub perm: CliOrganizationPermission,

    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Organization permission
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliOrganizationPermission {
    View,
    Create,
    Edit,
    Delete,
    Manage,
    ViewRole,
    CreateRole,
    EditRole,
    DeleteRole,
}
