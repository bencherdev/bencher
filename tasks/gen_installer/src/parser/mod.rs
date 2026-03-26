use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskTemplate {
    /// Product to generate installer for (cli or runner)
    pub product: Option<TaskProduct>,
    /// Template kind to generate (sh or ps1)
    pub template: Option<TaskTemplateKind>,
}

/// Product
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskProduct {
    /// CLI installer
    Cli,
    /// Runner installer
    Runner,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskTemplateKind {
    /// Shell installer
    Sh,
    /// Powershell installer
    Ps1,
}
