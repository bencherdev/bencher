use bencher_json::ResourceId;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use crate::cli::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliToken {
    /// List tokens
    #[clap(alias = "ls")]
    List(CliTokenList),
    /// Create a token
    #[clap(alias = "add")]
    Create(CliTokenCreate),
    /// View a token
    View(CliTokenView),
}

#[derive(Parser, Debug)]
pub struct CliTokenList {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTokenCreate {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Time to live (TTL)
    #[clap(long)]
    pub ttl: u64,

    /// Token name
    pub name: String,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTokenView {
    /// User slug or UUID
    #[clap(long)]
    pub user: ResourceId,

    /// Token UUID
    pub uuid: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
