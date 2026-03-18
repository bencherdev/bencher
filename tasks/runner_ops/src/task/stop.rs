use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskStop;

#[derive(Debug)]
pub struct Stop {
    ssh: Ssh,
}

impl TryFrom<TaskStop> for Stop {
    type Error = anyhow::Error;

    fn try_from(task: TaskStop) -> anyhow::Result<Self> {
        let TaskStop {
            name,
            server,
            key,
            user,
        } = task;
        let (server, key, user) = merge_ssh(name.as_deref(), server, key, user)?;
        Ok(Self {
            ssh: Ssh::new(server, key, user),
        })
    }
}

impl Stop {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { ssh } = self;
        stop_service(&ssh)?;
        Ok(())
    }
}

/// Stop the runner service if it is currently running.
pub fn stop_service(ssh: &Ssh) -> anyhow::Result<()> {
    if ssh.check("systemctl is-active --quiet bencher-runner")? {
        println!("Stopping runner service...");
        ssh.run("systemctl stop bencher-runner")?;
        println!("Runner service stopped");
    }
    Ok(())
}
