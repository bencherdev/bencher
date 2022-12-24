use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliDocs {
    /// File output path
    #[clap(long)]
    pub path: Option<String>,

    /// Documentation format
    pub format: Option<CliDocsFmt>,
}

/// Documentation format
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliDocsFmt {
    /// man page
    Man,
    /// HTML
    Html,
}
