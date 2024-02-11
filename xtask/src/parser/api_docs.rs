use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct TaskApiDocs {
    /// OpenAPI spec path
    #[clap(long)]
    pub spec: Option<Utf8PathBuf>,
    /// Docs output path
    #[clap(long)]
    pub path: Option<Utf8PathBuf>,
}
