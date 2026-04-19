use std::fmt::Write as _;

use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskLogs;
use crate::parser::server::load_server;

#[derive(Debug)]
pub struct Logs {
    ssh: Ssh,
    lines: Option<u32>,
    follow: bool,
}

impl TryFrom<TaskLogs> for Logs {
    type Error = anyhow::Error;

    fn try_from(task: TaskLogs) -> anyhow::Result<Self> {
        let TaskLogs {
            runner,
            server,
            ssh,
            user,
            lines,
            follow,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        Ok(Self {
            ssh: Ssh::new(server, ssh, user),
            lines,
            follow,
        })
    }
}

impl Logs {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { ssh, lines, follow } = self;
        let mut cmd = String::from("journalctl -u bencher-runner --no-pager");
        if let Some(n) = lines {
            let _ = write!(cmd, " -n {n}");
        }
        if follow {
            cmd.push_str(" -f");
        }
        ssh.exec(&cmd)
    }
}
