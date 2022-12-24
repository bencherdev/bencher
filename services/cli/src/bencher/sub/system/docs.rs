use std::path::{Path, PathBuf};

use async_trait::async_trait;
use clap::CommandFactory;

use crate::{
    bencher::sub::SubCmd,
    cli::docs::{CliDocs, CliDocsFmt},
    cli::CliBencher,
    CliError,
};

#[derive(Debug)]
pub struct Docs {
    format: Fmt,
    path: PathBuf,
}

#[derive(Debug)]
pub enum Fmt {
    Man,
    Html,
}

impl From<CliDocs> for Docs {
    fn from(docs: CliDocs) -> Self {
        Self {
            format: docs.format.into(),
            path: unwrap_path(docs.path),
        }
    }
}

impl From<Option<CliDocsFmt>> for Fmt {
    fn from(format: Option<CliDocsFmt>) -> Self {
        match format {
            Some(CliDocsFmt::Man) | None => Self::Man,
            Some(CliDocsFmt::Html) => Self::Html,
        }
    }
}

fn unwrap_path(path: Option<String>) -> PathBuf {
    let path = path.unwrap_or_default();
    Path::new(&path).into()
}

#[async_trait]
impl SubCmd for Docs {
    async fn exec(&self) -> Result<(), CliError> {
        match self.format {
            Fmt::Man => {
                let cmd = CliBencher::command();
                let man = clap_mangen::Man::new(cmd);
                let mut buffer: Vec<u8> = Default::default();
                man.render(&mut buffer)?;

                std::fs::write("out.1", buffer)?;

                Ok(())
            },
            Fmt::Html => Ok(()),
        }
    }
}
