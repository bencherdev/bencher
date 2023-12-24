use bencher_json::{ResourceId, ResourceName, TokenUuid};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliToken {
    /// List tokens
    #[clap(alias = "ls")]
    List(CliTokenList),
    /// Create a token
    #[clap(alias = "add")]
    Create(CliTokenCreate),
    /// View a token
    #[clap(alias = "get")]
    View(CliTokenView),
    // Update a token
    #[clap(alias = "edit")]
    Update(CliTokenUpdate),
}

#[derive(Parser, Debug)]
pub struct CliTokenList {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Token name
    #[clap(long)]
    pub name: Option<ResourceName>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliTokensSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliTokensSort {
    /// Name of the API token
    Name,
}

#[derive(Parser, Debug)]
pub struct CliTokenCreate {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Token name
    #[clap(long)]
    pub name: ResourceName,

    /// Time to live (seconds)
    #[clap(long)]
    pub ttl: Option<u32>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTokenView {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Token UUID
    pub token: TokenUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTokenUpdate {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Token name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Token UUID
    pub token: TokenUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
