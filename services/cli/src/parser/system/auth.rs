use bencher_json::{Email, Jwt, OrganizationUuid, Slug, UserName};
use clap::{Parser, Subcommand};

use crate::parser::CliBackend;
#[cfg(feature = "plus")]
use crate::parser::organization::plan::CliPlanLevel;

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    Signup(CliAuthSignup),
    // Log in to a user account
    Login(CliAuthLogin),
    // Confirm token
    Confirm(CliAuthConfirm),
    // Accept invite
    Accept(CliAuthAccept),
}

#[expect(clippy::doc_markdown)]
#[derive(Parser, Debug)]
pub struct CliAuthSignup {
    /// User email
    pub email: Email,

    /// User name
    #[clap(long)]
    pub name: UserName,

    /// User slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[cfg(feature = "plus")]
    /// Plan level
    #[clap(long)]
    pub plan: Option<CliPlanLevel>,

    /// User invitation JWT (JSON Web Token)
    #[clap(long)]
    pub invite: Option<Jwt>,

    /// Organization UUID
    #[clap(long, value_name = "UUID")]
    pub claim: Option<OrganizationUuid>,

    /// I agree to the Bencher Terms of Use (https://bencher.dev/legal/terms-of-use), Privacy Policy (https://bencher.dev/legal/privacy), and License Agreement (https://bencher.dev/legal/license)
    #[clap(long, required = true)]
    pub i_agree: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthLogin {
    /// User email
    pub email: Email,

    #[cfg(feature = "plus")]
    /// Plan level
    #[clap(long)]
    pub plan: Option<CliPlanLevel>,

    /// User invitation JWT (JSON Web Token)
    #[clap(long)]
    pub invite: Option<Jwt>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthConfirm {
    /// Email confirmation JWT (JSON Web Token)
    pub confirm: Jwt,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthAccept {
    /// Organization membership invitation JWT (JSON Web Token)
    pub invite: Jwt,

    #[clap(flatten)]
    pub backend: CliBackend,
}
