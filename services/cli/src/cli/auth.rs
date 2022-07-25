use clap::{
    Parser,
    Subcommand,
};

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    Signup(CliAuthSignup),
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
    #[clap(long)]
    pub email: String,

    /// Backend host URL (default http://api.bencher.dev)
    #[clap(long)]
    pub url: Option<String>,
}
