use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use crate::parser::CliBackend;

pub mod token;

#[derive(Subcommand, Debug)]
pub enum CliUser {
    /// View a user
    View(CliUserView),
}

#[derive(Parser, Debug)]
pub struct CliUserView {
    /// User slug or UUID
    pub user: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
