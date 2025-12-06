#![cfg(feature = "plus")]

use bencher_json::{NonEmpty, OrganizationResourceId, SsoUuid};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliSso {
    /// List organization SSO domains
    #[clap(alias = "ls")]
    List(CliSsoList),
    /// Create organization SSO domain
    #[clap(alias = "add")]
    Create(CliSsoCreate),
    /// View an organization SSO domain
    #[clap(alias = "get")]
    View(CliSsoView),
    /// Remove an organization SSO domain
    #[clap(alias = "rm")]
    Delete(CliSsoDelete),
}

#[derive(Parser, Debug)]
pub struct CliSsoList {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub pagination: CliPagination<CliSsoSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliSsoSort {
    /// SSO domain
    Domain,
}

#[derive(Parser, Debug)]
pub struct CliSsoCreate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// SSO domain
    #[clap(long)]
    pub domain: NonEmpty,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliSsoView {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// SSO UUID
    pub sso: SsoUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliSsoDelete {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// SSO UUID
    pub sso: SsoUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
