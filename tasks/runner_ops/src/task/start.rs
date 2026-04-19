use bencher_json::{RunnerResourceId, Secret};

use super::merge_ssh_with_extras;
use super::ssh::Ssh;
use crate::parser::TaskStart;

#[derive(Debug)]
pub struct Start {
    ssh: Ssh,
    host: url::Url,
    runner: RunnerResourceId,
    runner_key: Secret,
    danger_allow_no_sandbox: bool,
}

impl TryFrom<TaskStart> for Start {
    type Error = anyhow::Error;

    fn try_from(task: TaskStart) -> anyhow::Result<Self> {
        let TaskStart {
            runner,
            server,
            key,
            user,
            runner_key,
            host,
            danger_allow_no_sandbox,
        } = task;
        let (ssh, host, runner, runner_key) =
            merge_ssh_with_extras(runner, server, key, user, runner_key, host)?;
        Ok(Self {
            ssh,
            host,
            runner,
            runner_key,
            danger_allow_no_sandbox,
        })
    }
}

impl Start {
    pub fn new(
        ssh: Ssh,
        host: url::Url,
        runner: RunnerResourceId,
        runner_key: Secret,
        danger_allow_no_sandbox: bool,
    ) -> Self {
        Self {
            ssh,
            host,
            runner,
            runner_key,
            danger_allow_no_sandbox,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            host,
            runner,
            runner_key,
            danger_allow_no_sandbox,
        } = self;
        println!("Configuring runner credentials...");
        ssh.run("mkdir -p /etc/systemd/system/bencher-runner.service.d")?;
        let no_sandbox_env = if danger_allow_no_sandbox {
            "Environment=BENCHER_DANGER_ALLOW_NO_SANDBOX=true\n"
        } else {
            ""
        };
        ssh.run(&format!(
            "cat > /etc/systemd/system/bencher-runner.service.d/credentials.conf << 'CRED_EOF'\n\
             [Service]\n\
             Environment=BENCHER_HOST={host}\n\
             Environment=BENCHER_RUNNER={runner}\n\
             Environment=BENCHER_RUNNER_KEY={runner_key}\n\
             {no_sandbox_env}\
             CRED_EOF",
            runner_key = runner_key.as_ref(),
        ))?;
        println!("Starting runner service...");
        ssh.run("systemctl daemon-reload")?;
        ssh.run("systemctl restart bencher-runner")?;
        ssh.run("systemctl status bencher-runner")?;
        println!("Runner is running");
        Ok(())
    }
}
