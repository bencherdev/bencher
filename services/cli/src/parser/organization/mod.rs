use bencher_json::{OrganizationResourceId, OrganizationSlug, ResourceName};
use clap::{Parser, Subcommand, ValueEnum};

use super::CliPagination;
#[cfg(feature = "plus")]
use super::ElidedOption;
use crate::parser::CliBackend;

pub mod claim;
pub mod member;
pub mod plan;
pub mod sso;
pub mod usage;

use claim::CliOrganizationClaim;
#[cfg(feature = "plus")]
use usage::CliOrganizationUsage;

#[derive(Subcommand, Debug)]
pub enum CliOrganization {
    /// List organizations
    #[clap(alias = "ls")]
    List(CliOrganizationList),
    /// Create an organization
    #[clap(alias = "add")]
    Create(CliOrganizationCreate),
    /// View an organization
    #[clap(alias = "get")]
    View(CliOrganizationView),
    /// Update an organization
    #[clap(alias = "edit")]
    Update(CliOrganizationUpdate),
    /// Delete an organization
    #[clap(alias = "rm")]
    Delete(CliOrganizationDelete),

    /// Claim an organization
    Claim(CliOrganizationClaim),

    /// Check organization permission
    Allowed(CliOrganizationAllowed),

    #[cfg(feature = "plus")]
    /// Check organization metrics usage
    Usage(CliOrganizationUsage),
}

#[derive(Parser, Debug)]
pub struct CliOrganizationList {
    /// Organization name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Organization search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliOrganizationsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliOrganizationsSort {
    /// Name of the organization
    Name,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationCreate {
    /// Organization name
    #[clap(long)]
    pub name: ResourceName,

    /// Organization slug
    #[clap(long)]
    pub slug: Option<OrganizationSlug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationView {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationUpdate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Organization name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Organization slug
    #[clap(long)]
    pub slug: Option<OrganizationSlug>,

    #[cfg(feature = "plus")]
    /// Organization license
    /// To remove the current license key without replacing it, use an underscore (`_`).
    #[clap(long, value_name = "KEY")]
    pub license: Option<ElidedOption<bencher_json::Jwt>>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationAllowed {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Organization permission
    #[clap(long)]
    pub perm: CliOrganizationPermission,

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

#[derive(Parser, Debug)]
pub struct CliOrganizationDelete {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
