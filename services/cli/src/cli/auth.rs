use clap::{Parser, Subcommand};

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
    /// Backend host URL (default https://api.bencher.dev)
    #[clap(long)]
    pub host: Option<String>,

    /// User name
    #[clap(long)]
    pub name: String,

    /// User slug
    #[clap(long)]
    pub slug: Option<String>,

    /// User email
    pub email: String,
}

#[derive(Parser, Debug)]
pub struct CliAuthLogin {
    /// Backend host URL (default https://api.bencher.dev)
    #[clap(long)]
    pub host: Option<String>,

    /// User email
    pub email: String,
}

#[derive(Parser, Debug)]
pub struct CliAuthConfirm {
    /// Backend host URL (default https://api.bencher.dev)
    #[clap(long)]
    pub host: Option<String>,

    /// Confirmation token
    pub token: String,
}
