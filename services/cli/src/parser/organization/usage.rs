#![cfg(feature = "plus")]

use bencher_json::OrganizationResourceId;
use clap::Parser;

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliOrganizationUsage {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
