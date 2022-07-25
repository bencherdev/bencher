use clap::{
    ArgGroup,
    Args,
    Parser,
    Subcommand,
    ValueEnum,
};

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    // Signup(CliAuthSignup),
}
