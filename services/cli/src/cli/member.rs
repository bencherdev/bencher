use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliMember {
    /// List organization members
    #[clap(alias = "ls")]
    List(CliMemberList),
    /// View an organization member
    View(CliMemberView),
}

#[derive(Parser, Debug)]
pub struct CliMemberList {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMemberView {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// User slug or UUID
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
