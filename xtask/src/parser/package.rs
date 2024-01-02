use camino::Utf8PathBuf;
use clap::Parser;

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
