use bencher_json::{ResourceName, UserKeyUuid, UserResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliUserKey {
    /// List user API keys
    #[clap(alias = "ls")]
    List(CliUserKeyList),
    /// Create a user API key
    #[clap(alias = "add")]
    Create(CliUserKeyCreate),
    /// View a user API key
    #[clap(alias = "get")]
    View(CliUserKeyView),
    /// Update a user API key
    #[clap(alias = "edit")]
    Update(CliUserKeyUpdate),
    /// Revoke a user API key
    #[clap(alias = "rm")]
    Revoke(CliUserKeyRevoke),
}

#[derive(Parser, Debug)]
pub struct CliUserKeyList {
    /// User slug or UUID
    pub user: UserResourceId,

    /// Key name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Key search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Show only revoked keys instead of active ones
    #[clap(long)]
    pub revoked: bool,

    #[clap(flatten)]
    pub pagination: CliPagination<CliUserKeysSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliUserKeysSort {
    /// Name of the API key
    Name,
}

#[derive(Parser, Debug)]
pub struct CliUserKeyCreate {
    /// User slug or UUID
    pub user: UserResourceId,

    /// Key name
    #[clap(long)]
    pub name: ResourceName,

    /// Time to live (seconds)
    #[clap(long)]
    pub ttl: Option<u32>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliUserKeyView {
    /// User slug or UUID
    pub user: UserResourceId,

    /// Key UUID
    pub uuid: UserKeyUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliUserKeyUpdate {
    /// User slug or UUID
    pub user: UserResourceId,

    /// Key UUID
    pub uuid: UserKeyUuid,

    /// Key name
    #[clap(long)]
    pub name: Option<ResourceName>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliUserKeyRevoke {
    /// User slug or UUID
    pub user: UserResourceId,

    /// Key UUID
    pub uuid: UserKeyUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
