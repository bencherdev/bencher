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
    pub user: ResourceId,

    /// Token name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Token search string
    #[clap(long)]
    pub search: Option<String>,

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
    pub user: ResourceId,

    /// Token UUID
    pub uuid: TokenUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTokenUpdate {
    /// User slug or UUID
    pub user: ResourceId,

    /// Token UUID
    pub uuid: TokenUuid,

    /// Token name
    #[clap(long)]
    pub name: Option<ResourceName>,

    #[clap(flatten)]
    pub backend: CliBackend,
}
