use bencher_json::{ResourceName, RunnerResourceId, RunnerSlug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliArchived, CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliRunner {
    /// List runners
    #[clap(alias = "ls")]
    List(CliRunnerList),
    /// Create a runner
    #[clap(alias = "add")]
    Create(CliRunnerCreate),
    /// View a runner
    #[clap(alias = "get")]
    View(CliRunnerView),
    /// Update a runner
    #[clap(alias = "edit")]
    Update(CliRunnerUpdate),
    /// Rotate a runner token
    Token(CliRunnerToken),
}

#[derive(Parser, Debug)]
pub struct CliRunnerList {
    /// Runner name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Runner search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Include archived runners
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub pagination: CliPagination<CliRunnersSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliRunnersSort {
    /// Name of the runner
    Name,
    /// Date time the runner was created
    Created,
}

#[derive(Parser, Debug)]
pub struct CliRunnerCreate {
    /// Runner name
    #[clap(long)]
    pub name: ResourceName,

    /// Runner slug
    #[clap(long)]
    pub slug: Option<RunnerSlug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliRunnerView {
    /// Runner slug or UUID
    pub runner: RunnerResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliRunnerUpdate {
    /// Runner slug or UUID
    pub runner: RunnerResourceId,

    /// Runner name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Runner slug
    #[clap(long)]
    pub slug: Option<RunnerSlug>,

    #[clap(flatten)]
    pub archived: CliArchived,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliRunnerToken {
    /// Runner slug or UUID
    pub runner: RunnerResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
