use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliMember {
    /// List organizations
    #[clap(alias = "ls")]
    List(CliMemberList),
    /// Create a organization
    #[clap(alias = "add")]
    Create(CliOrganizationCreate),
    /// View a organization
    View(CliOrganizationView),
}

#[derive(Parser, Debug)]
pub struct CliMemberList {
    /// Organization slug or UUID
    pub org: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMemberView {
    /// Organization slug or UUID
    pub org: ResourceId,

    /// User slug or UUID
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
