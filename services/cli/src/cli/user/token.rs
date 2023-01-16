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

    /// Token name
    pub name: String,

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
    pub uuid: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
