use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliReleaseNotes {
    /// Changelog path
    #[clap(long)]
    pub changelog: Option<Utf8PathBuf>,

    /// File output path
    #[clap(long)]
    pub path: Option<Utf8PathBuf>,
}
