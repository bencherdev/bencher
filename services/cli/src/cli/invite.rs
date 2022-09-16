use clap::{Parser, ValueEnum};
use uuid::Uuid;

use super::CliBackend;

#[derive(Parser, Debug)]
pub struct CliInvite {
    /// Email for the invitation
    #[clap(long)]
    pub email: String,

    /// Organization slug or UUID
    #[clap(long)]
    pub org: Uuid,

    /// Organization role
    #[clap(value_enum, long)]
    pub role: CliInviteRole,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Role within the organization
#[derive(ValueEnum, Debug, Clone)]
pub enum CliInviteRole {
    Member,
    Leader,
}
