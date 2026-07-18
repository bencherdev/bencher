use bencher_json::{RunnerResourceId, Secret, UpdateChannel};

use super::merge_ssh_with_extras;
use super::ssh::Ssh;
use crate::parser::TaskStart;

#[derive(Debug)]
pub struct Start {
    ssh: Ssh,
    host: url::Url,
    runner: RunnerResourceId,
    key: Secret,
    update_channel: Option<UpdateChannel>,
    danger_allow_no_sandbox: bool,
}

impl TryFrom<TaskStart> for Start {
    type Error = anyhow::Error;

    fn try_from(task: TaskStart) -> anyhow::Result<Self> {
        let TaskStart {
            runner,
            server,
            ssh,
            user,
            key,
            host,
            update_channel,
            danger_allow_no_sandbox,
        } = task;
        let (ssh, host, runner, key, update_channel) =
            merge_ssh_with_extras(runner, server, ssh, user, key, host, update_channel)?;
        Ok(Self {
            ssh,
            host,
            runner,
            key,
            update_channel,
            danger_allow_no_sandbox,
        })
    }
}

impl Start {
    pub fn new(
        ssh: Ssh,
        host: url::Url,
        runner: RunnerResourceId,
        key: Secret,
        update_channel: Option<UpdateChannel>,
        danger_allow_no_sandbox: bool,
    ) -> Self {
        Self {
            ssh,
            host,
            runner,
            key,
            update_channel,
            danger_allow_no_sandbox,
        }
    }

    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            host,
            runner,
            key,
            update_channel,
            danger_allow_no_sandbox,
        } = self;
        println!("Configuring runner credentials...");
        ssh.run("mkdir -p /etc/systemd/system/bencher-runner.service.d")?;
        let credentials = credentials_conf(
            &host,
            &runner,
            key.as_ref(),
            update_channel,
            danger_allow_no_sandbox,
        );
        ssh.run(&format!(
            "cat > /etc/systemd/system/bencher-runner.service.d/credentials.conf << 'CRED_EOF'\n\
             {credentials}\
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

/// Build the contents of the systemd credentials drop-in.
fn credentials_conf(
    host: &url::Url,
    runner: &RunnerResourceId,
    key: &str,
    update_channel: Option<UpdateChannel>,
    danger_allow_no_sandbox: bool,
) -> String {
    let channel_env = update_channel.map_or_else(String::new, |channel| {
        format!("Environment=BENCHER_UPDATE_CHANNEL={channel}\n")
    });
    let no_sandbox_env = if danger_allow_no_sandbox {
        "Environment=BENCHER_DANGER_ALLOW_NO_SANDBOX=true\n"
    } else {
        ""
    };
    format!(
        "[Service]\n\
         Environment=BENCHER_HOST={host}\n\
         Environment=BENCHER_RUNNER={runner}\n\
         Environment=BENCHER_RUNNER_KEY={key}\n\
         {channel_env}\
         {no_sandbox_env}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_host() -> url::Url {
        "https://api.example.com".parse().unwrap()
    }

    fn test_runner() -> RunnerResourceId {
        "test-runner".parse().unwrap()
    }

    #[test]
    fn credentials_conf_minimal() {
        let conf = credentials_conf(&test_host(), &test_runner(), "secret-key", None, false);
        assert_eq!(
            conf,
            "[Service]\n\
             Environment=BENCHER_HOST=https://api.example.com/\n\
             Environment=BENCHER_RUNNER=test-runner\n\
             Environment=BENCHER_RUNNER_KEY=secret-key\n"
        );
    }

    #[test]
    fn credentials_conf_with_channel() {
        let conf = credentials_conf(
            &test_host(),
            &test_runner(),
            "secret-key",
            Some(UpdateChannel::Canary),
            false,
        );
        assert!(conf.contains("Environment=BENCHER_UPDATE_CHANNEL=canary\n"));
    }

    #[test]
    fn credentials_conf_with_no_sandbox() {
        let conf = credentials_conf(&test_host(), &test_runner(), "secret-key", None, true);
        assert!(conf.contains("Environment=BENCHER_DANGER_ALLOW_NO_SANDBOX=true\n"));
    }
}
