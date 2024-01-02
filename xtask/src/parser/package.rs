use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliMan {
    /// File output path
    #[clap(long)]
    pub path: Option<String>,

    /// File output name
    #[clap(long)]
    pub name: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CliDeb {
    /// CLI bin path
    pub bin: Utf8PathBuf,
    /// .deb build directory
    #[clap(long)]
    pub dir: Utf8PathBuf,
    /// Target architecture
    #[clap(long)]
    pub arch: String,
}
