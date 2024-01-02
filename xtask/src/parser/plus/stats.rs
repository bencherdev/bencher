use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliStats {
    /// Stats JSON
    pub stats: String,
}
