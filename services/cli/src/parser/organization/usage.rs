#![cfg(feature = "plus")]

use bencher_json::ResourceId;
use clap::Parser;

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliOrganizationUsage {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
