use bencher_cli::CliBencher;
use camino::Utf8PathBuf;
use clap::CommandFactory;

use crate::parser::TaskMan;

const BIN_NAME: &str = "bencher";
const MAN_EXTENSION: &str = "1";

#[derive(Debug)]
pub struct Man {
    path: Utf8PathBuf,
    name: Utf8PathBuf,
}

impl From<TaskMan> for Man {
    fn from(man: TaskMan) -> Self {
        let TaskMan { path, name } = man;
        Self {
            path,
            name: unwrap_name(name),
        }
    }
}

fn unwrap_name(name: Option<Utf8PathBuf>) -> Utf8PathBuf {
    name.unwrap_or_else(|| BIN_NAME.into())
}

impl Man {
    pub fn exec(&self) -> anyhow::Result<()> {
        let cmd = CliBencher::command();
        let man = clap_mangen::Man::new(cmd);
        let mut buffer = Vec::default();
        man.render(&mut buffer)?;

        let mut path = self.path.clone();
        path.push(&self.name);
        path.set_extension(MAN_EXTENSION);
        std::fs::write(path, buffer).map_err(Into::into)
    }
}
