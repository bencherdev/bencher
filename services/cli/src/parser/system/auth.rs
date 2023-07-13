use bencher_json::{Email, Jwt, Slug, UserName};
use clap::{Parser, Subcommand};

#[cfg(feature = "plus")]
use crate::parser::organization::plan::CliPlanLevel;
use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    Signup(CliAuthSignup),
    // Log in to a user account
    Login(CliAuthLogin),
    // Confirm token
    Confirm(CliAuthConfirm),
}

#[derive(Parser, Debug)]
pub struct CliAuthSignup {
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

    /// User email
    pub email: Email,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthLogin {
    #[cfg(feature = "plus")]
    /// Plan level
    #[clap(long)]
    pub plan: Option<CliPlanLevel>,

    /// User invitation JWT (JSON Web Token)
    #[clap(long)]
    pub invite: Option<Jwt>,

    /// User email
    pub email: Email,

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
