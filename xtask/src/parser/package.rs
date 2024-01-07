use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct TaskMan {
    /// File output path
    #[clap(long)]
    pub path: Utf8PathBuf,

    /// File output name
    #[clap(long)]
    pub name: Option<Utf8PathBuf>,
}

#[cfg(target_os = "linux")]
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
