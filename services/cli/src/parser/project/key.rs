use bencher_json::{ProjectKeyUuid, ProjectResourceId, ResourceName};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliPagination, CliProjectBackend};

#[derive(Subcommand, Debug)]
pub enum CliProjectKey {
    /// List project keys
    #[clap(alias = "ls")]
    List(CliProjectKeyList),
    /// Create a project key
    #[clap(alias = "add")]
    Create(CliProjectKeyCreate),
    /// View a project key
    #[clap(alias = "get")]
    View(CliProjectKeyView),
    /// Update a project key
    #[clap(alias = "edit")]
    Update(CliProjectKeyUpdate),
    /// Revoke a project key
    #[clap(alias = "rm")]
    Revoke(CliProjectKeyRevoke),
}

#[derive(Parser, Debug)]
pub struct CliProjectKeyList {
    /// Project slug or UUID
    pub project: ProjectResourceId,

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
    pub pagination: CliPagination<CliProjectKeysSort>,

    #[clap(flatten)]
    pub backend: CliProjectBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliProjectKeysSort {
    /// Name of the project key
    Name,
}

#[derive(Parser, Debug)]
pub struct CliProjectKeyCreate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Key name
    #[clap(long)]
    pub name: ResourceName,

    /// Time to live (seconds)
    #[clap(long)]
    pub ttl: Option<u32>,

    #[clap(flatten)]
    pub backend: CliProjectBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectKeyView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Project key UUID
    pub uuid: ProjectKeyUuid,

    #[clap(flatten)]
    pub backend: CliProjectBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectKeyUpdate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Project key UUID
    pub uuid: ProjectKeyUuid,

    /// Key name
    #[clap(long)]
    pub name: Option<ResourceName>,

    #[clap(flatten)]
    pub backend: CliProjectBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectKeyRevoke {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Project key UUID
    pub uuid: ProjectKeyUuid,

    #[clap(flatten)]
    pub backend: CliProjectBackend,
}
