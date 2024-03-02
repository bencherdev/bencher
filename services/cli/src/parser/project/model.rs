use bencher_json::{ModelUuid, ResourceId};
use clap::{Parser, Subcommand};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliModel {
    /// View a threshold model
    #[clap(alias = "get")]
    View(CliModelView),
}

#[derive(Parser, Debug)]
pub struct CliModelView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Model UUID
    pub model: ModelUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
