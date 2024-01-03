use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskTemplate {
    /// Documentation format
    pub template: Option<TaskTemplateKind>,
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
