use super::merge_ssh_with_extras;
use super::ssh::Ssh;
use crate::parser::TaskStart;

#[derive(Debug)]
pub struct Start {
    ssh: Ssh,
    host: url::Url,
    runner: String,
    token: String,
}

impl TryFrom<TaskStart> for Start {
    type Error = anyhow::Error;

    fn try_from(task: TaskStart) -> anyhow::Result<Self> {
        let TaskStart {
            name,
            server,
            key,
            user,
            runner,
            token,
            host,
        } = task;
        let (ssh, host, runner, token) =
            merge_ssh_with_extras(name.as_deref(), server, key, user, runner, token, host)?;
        Ok(Self {
            ssh,
            host,
            runner,
            token,
        })
    }
}

impl Start {
    pub fn new(ssh: Ssh, host: url::Url, runner: String, token: String) -> Self {
        Self {
            ssh,
            host,
            runner,
            token,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            host,
            runner,
            token,
        } = self;
        println!("Configuring runner credentials...");
        ssh.run("mkdir -p /etc/systemd/system/bencher-runner.service.d")?;
        ssh.run(&format!(
            "cat > /etc/systemd/system/bencher-runner.service.d/credentials.conf << 'CRED_EOF'\n\
             [Service]\n\
             Environment=BENCHER_HOST={host}\n\
             Environment=BENCHER_RUNNER={runner}\n\
             Environment=BENCHER_RUNNER_TOKEN={token}\n\
             CRED_EOF"
        ))?;
        println!("Starting runner service...");
        ssh.run("systemctl daemon-reload")?;
        ssh.run("systemctl restart bencher-runner")?;
        ssh.run("systemctl status bencher-runner")?;
        println!("Runner is running");
        Ok(())
    }
}
