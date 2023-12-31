#![cfg(feature = "docs")]

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use clap::CommandFactory;

use crate::{
    bencher::sub::SubCmd,
    parser::docs::{CliDocs, CliDocsFmt},
    parser::CliBencher,
    CliError,
};

const BIN_NAME: &str = "bencher";
const MAN_EXTENSION: &str = "1";

#[derive(Debug)]
pub struct Docs {
    format: Fmt,
    path: PathBuf,
    name: String,
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
            name: unwrap_name(docs.name),
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

fn unwrap_name(name: Option<String>) -> String {
    name.unwrap_or_else(|| BIN_NAME.into())
}

#[async_trait]
impl SubCmd for Docs {
    async fn exec(&self) -> Result<(), CliError> {
        match self.format {
            Fmt::Man => {
                let cmd = CliBencher::command();
                let man = clap_mangen::Man::new(cmd);
                let mut buffer = Vec::default();
                man.render(&mut buffer).map_err(CliError::Docs)?;

                let mut path = self.path.clone();
                path.push(&self.name);
                path.set_extension(MAN_EXTENSION);
                std::fs::write(path, buffer).map_err(CliError::Docs)
            },
            Fmt::Html => Ok(()),
        }
    }
}
