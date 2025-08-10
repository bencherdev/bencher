use bencher_json::OrganizationResourceId;
use clap::Parser;

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliOrganizationClaim {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
