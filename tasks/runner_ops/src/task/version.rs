use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskVersion;
use crate::parser::server::load_server;

#[derive(Debug)]
pub struct Version {
    ssh: Ssh,
}

impl TryFrom<TaskVersion> for Version {
    type Error = anyhow::Error;

    fn try_from(task: TaskVersion) -> anyhow::Result<Self> {
        let TaskVersion {
            runner,
            server,
            ssh,
            user,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        Ok(Self {
            ssh: Ssh::new(server, ssh, user),
        })
    }
}

impl Version {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { ssh } = self;
        ssh.run("/usr/local/bin/runner --version")?;
        Ok(())
    }
}
