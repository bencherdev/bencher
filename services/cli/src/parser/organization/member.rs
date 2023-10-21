use bencher_json::{Email, ResourceId, UserName};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliMember {
    /// List organization members
    #[clap(alias = "ls")]
    List(CliMemberList),
    /// Invite an organization member
    Invite(CliMemberInvite),
    /// View an organization member
    #[clap(alias = "get")]
    View(CliMemberView),
    /// Update an organization member
    #[clap(alias = "edit")]
    Update(CliMemberUpdate),
    /// Remove an organization member
    #[clap(alias = "rm")]
    Remove(CliMemberRemove),
}

#[derive(Parser, Debug)]
pub struct CliMemberList {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Member name
    #[clap(long)]
    pub name: Option<UserName>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliMembersSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliMembersSort {
    /// Name of the member
    Name,
}

#[derive(Parser, Debug)]
pub struct CliMemberInvite {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Name of user for invitation (optional)
    #[clap(long)]
    pub name: Option<UserName>,

    /// Email for the invitation
    #[clap(long)]
    pub email: Email,

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

#[derive(Parser, Debug)]
pub struct CliMemberRemove {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// User slug or UUID
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Role within the organization
#[derive(ValueEnum, Debug, Clone)]
pub enum CliMemberRole {
    // TODO Team Management
    // Member,
    Leader,
}
