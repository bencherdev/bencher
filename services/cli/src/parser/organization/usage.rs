#![cfg(feature = "plus")]

use bencher_json::{DateTime, ResourceId};
use clap::Parser;

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliOrganizationUsage {
    /// Organization slug or UUID
    pub organization: ResourceId,

    /// Start time (seconds since epoch)
    #[clap(long)]
    pub start: DateTime,

    /// End time (seconds since epoch)
    #[clap(long)]
    pub end: DateTime,

    #[clap(flatten)]
    pub backend: CliBackend,
}
