use bencher_json::ResourceId;
use clap::Parser;

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliOrganizationClaim {
    /// Organization slug or UUID
    pub organization: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
