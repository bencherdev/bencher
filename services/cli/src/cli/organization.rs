use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliOrganization {
    /// List organizations
    #[clap(alias = "ls")]
    List(CliOrganizationList),
    /// Create a organization
    #[clap(alias = "add")]
    Create(CliOrganizationCreate),
    /// View a organization
    View(CliOrganizationView),
}

#[derive(Parser, Debug)]
pub struct CliOrganizationList {
    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationCreate {
    /// Organization name
    pub name: String,

    /// Organization slug
    #[clap(long)]
    pub slug: Option<String>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliOrganizationView {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
