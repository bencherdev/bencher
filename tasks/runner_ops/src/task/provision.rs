use camino::Utf8PathBuf;

use super::merge_ssh;
use super::setup;
use super::ssh::Ssh;
use crate::parser::TaskProvision;
use crate::parser::server::load_server;

#[derive(Debug)]
pub struct Provision {
    ssh: Ssh,
    runner_binary: Option<Utf8PathBuf>,
}

impl TryFrom<TaskProvision> for Provision {
    type Error = anyhow::Error;

    fn try_from(task: TaskProvision) -> anyhow::Result<Self> {
        let TaskProvision {
            runner,
            server,
            key,
            user,
            runner_binary,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let (server, key, user) = merge_ssh(file.as_ref(), server, key, user)?;
        Ok(Self {
            ssh: Ssh::new(server, key, user),
            runner_binary,
        })
    }
}

impl Provision {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { ssh, runner_binary } = self;
        super::install_os::install_os(&ssh)?;
        super::harden::harden(&ssh)?;
        setup::deploy(&ssh, runner_binary.as_deref())?;
        Ok(())
    }
}
