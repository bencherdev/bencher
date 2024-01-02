use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliTemplate {
    /// Documentation format
    pub template: CliTemplateKind,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliTemplateKind {
    /// Shell installer
    Sh,
    /// Powershell installer
    Ps1,
}
