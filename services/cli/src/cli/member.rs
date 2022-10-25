use bencher_json::ResourceId;
use clap::{Parser, Subcommand, ValueEnum};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliMember {
    /// List organization members
    #[clap(alias = "ls")]
    List(CliMemberList),
    /// Invite an organization member
    Invite(CliMemberInvite),
    /// View an organization member
    View(CliMemberView),
    /// Update an organization member
    #[clap(alias = "edit")]
    Update(CliMemberUpdate),
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
pub struct CliMemberInvite {
    /// Name of user for invitation (optional)
    #[clap(long)]
    pub name: Option<String>,

    /// Email for the invitation
    #[clap(long)]
    pub email: String,

    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Member role
    #[clap(value_enum, long)]
    pub role: CliMemberRole,

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

#[derive(Parser, Debug)]
pub struct CliMemberUpdate {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// User slug or UUID
    pub user: ResourceId,

    /// Member role
    #[clap(value_enum, long)]
    pub role: Option<CliMemberRole>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Role within the organization
#[derive(ValueEnum, Debug, Clone)]
pub enum CliMemberRole {
    Member,
    Leader,
}
