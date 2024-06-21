use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Generate types
    Types(TaskTypes),
    /// Generate API spec
    Spec(TaskSpec),
    /// Generate TypeScript types
    Ts(TaskTs),
}

#[derive(Parser, Debug)]
pub struct TaskTypes {}

#[derive(Parser, Debug)]
pub struct TaskSpec {}

#[derive(Parser, Debug)]
pub struct TaskTs {}
