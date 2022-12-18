use clap::{Parser, Subcommand};

use crate::cli::CliBackend;

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
    pub name: String,

    /// User slug
    #[clap(long)]
    pub slug: Option<String>,

    /// User invitation JWT (JSON Web Token)
    #[clap(long)]
    pub invite: Option<String>,

    /// User email
    pub email: String,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthLogin {
    /// User invitation JWT (JSON Web Token)
    #[clap(long)]
    pub invite: Option<String>,

    /// User email
    pub email: String,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliAuthConfirm {
    #[clap(flatten)]
    pub backend: CliBackend,
}
