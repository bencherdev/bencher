use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Create CLI man page
    Man(TaskMan),
    /// Create CLI .deb
    Deb(TaskDeb),
}

#[derive(Parser, Debug)]
pub struct TaskMan {
    /// File output path
    #[clap(long)]
    pub path: Utf8PathBuf,

    /// File output name
    #[clap(long)]
    pub name: Option<Utf8PathBuf>,
}

#[derive(Parser, Debug)]
pub struct TaskDeb {
    /// CLI bin path
    pub bin: Utf8PathBuf,
    /// .deb build directory
    #[clap(long)]
    pub dir: Utf8PathBuf,
    /// Target architecture
    #[clap(long)]
    pub arch: String,
}
