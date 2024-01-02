use std::path::{Path, PathBuf};

use clap::CommandFactory;

use crate::parser::CliMan;

const BIN_NAME: &str = "bencher";
const MAN_EXTENSION: &str = "1";

#[derive(Debug)]
pub struct Man {
    path: PathBuf,
    name: String,
}

impl From<CliMan> for Man {
    fn from(docs: CliMan) -> Self {
        Self {
            path: unwrap_path(docs.path),
            name: unwrap_name(docs.name),
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

impl Man {
    fn exec(&self) -> anyhow::Result<()> {
        let cmd = CliBencher::command();
        let man = clap_mangen::Man::new(cmd);
        let mut buffer = Vec::default();
        man.render(&mut buffer).map_err(CliError::Man)?;

        let mut path = self.path.clone();
        path.push(&self.name);
        path.set_extension(MAN_EXTENSION);
        std::fs::write(path, buffer).map_err(CliError::Man)
    }
}
