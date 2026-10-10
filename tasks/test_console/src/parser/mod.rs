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
    /// Run the console end-to-end suite against a fresh, seeded API
    #[cfg(feature = "plus")]
    E2e(TaskE2e),
}

#[derive(Parser, Debug)]
pub struct TaskTestConsole {
    pub ref_name: String,

    #[clap(long)]
    pub agent_key: Option<String>,
}

#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct TaskE2e {
    /// Serve the console build already in `services/console/dist`
    #[clap(long)]
    pub skip_console_build: bool,

    /// Arguments passed through to Playwright, after `--`
    #[clap(last = true)]
    pub playwright: Vec<String>,
}
