use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    Dev(TaskTestConsole),
    Prod(TaskTestConsole),
}

#[derive(Parser, Debug)]
pub struct TaskTestConsole {
    pub ref_name: String,

    #[clap(long)]
    pub user_agent: Option<String>,
}
