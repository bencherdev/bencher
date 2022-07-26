use clap::{
    Parser,
    Subcommand,
};

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    Signup(CliAuthSignup),
    // Log in to a user account
    Login(CliAuthLogin),
}

#[derive(Parser, Debug)]
pub struct CliAuthSignup {
    /// User name
    #[clap(long)]
    pub name: String,

    /// User slug
    #[clap(long)]
    pub slug: Option<String>,

    /// User email
    pub email: String,

    /// Backend host URL (default http://api.bencher.dev)
    #[clap(long)]
    pub url: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CliAuthLogin {
    /// User email
    pub email: String,

    /// Backend host URL (default http://api.bencher.dev)
    #[clap(long)]
    pub url: Option<String>,
}
