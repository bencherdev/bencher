use bencher_json::{RunnerResourceId, Secret, UpdateChannel};

use super::merge_ssh_with_extras;
use super::ssh::Ssh;
use crate::parser::TaskStart;

const DROP_IN_DIR: &str = "/etc/systemd/system/bencher-runner.service.d";

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
        let credentials = credentials_conf(
            &host,
            &runner,
            key.as_ref(),
            update_channel,
            danger_allow_no_sandbox,
        );
        run_steps(
            start_steps(&credentials),
            |command| ssh.run(command),
            |command| ssh.run_quiet(command),
        )?;
        println!("Runner is running");
        Ok(())
    }
}

/// Run each step through `printed`, or through `quiet` when it must not be printed.
fn run_steps(
    steps: impl IntoIterator<Item = Step>,
    mut printed: impl FnMut(&str) -> anyhow::Result<String>,
    mut quiet: impl FnMut(&str) -> anyhow::Result<String>,
) -> anyhow::Result<()> {
    for Step {
        notice,
        command,
        echo,
    } in steps
    {
        if let Some(notice) = notice {
            println!("{notice}");
        }
        match echo {
            Echo::Printed => printed(&command)?,
            Echo::Quiet => quiet(&command)?,
        };
    }
    Ok(())
}

/// A remote command that `start` runs and the line it prints first.
struct Step {
    notice: Option<&'static str>,
    command: String,
    echo: Echo,
}

/// Whether a remote command and its output may be printed.
enum Echo {
    Printed,
    Quiet,
}

/// The credentials carry the runner key, so writing them is the one quiet step.
fn start_steps(credentials: &str) -> [Step; 5] {
    [
        Step {
            notice: Some("Configuring runner credentials..."),
            command: format!("mkdir -p {DROP_IN_DIR}"),
            echo: Echo::Printed,
        },
        Step {
            notice: Some("Writing runner credentials..."),
            command: format!(
                "cat > {DROP_IN_DIR}/credentials.conf << 'CRED_EOF'\n{credentials}CRED_EOF"
            ),
            echo: Echo::Quiet,
        },
        Step {
            notice: Some("Starting runner service..."),
            command: "systemctl daemon-reload".to_owned(),
            echo: Echo::Printed,
        },
        Step {
            notice: None,
            command: "systemctl restart bencher-runner".to_owned(),
            echo: Echo::Printed,
        },
        Step {
            notice: None,
            command: "systemctl status bencher-runner".to_owned(),
            echo: Echo::Printed,
        },
    ]
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

    #[test]
    fn start_never_prints_the_key() {
        let credentials = credentials_conf(
            &test_host(),
            &test_runner(),
            "secret-key",
            Some(UpdateChannel::Canary),
            true,
        );
        let mut printed = Vec::new();
        let mut quiet = Vec::new();
        run_steps(
            start_steps(&credentials),
            |command| {
                printed.push(command.to_owned());
                Ok(String::new())
            },
            |command| {
                quiet.push(command.to_owned());
                Ok(String::new())
            },
        )
        .unwrap();
        assert!(
            quiet.iter().any(|command| command.contains("secret-key")),
            "the credentials must still be written"
        );
        assert!(
            printed
                .iter()
                .all(|command| !command.contains("secret-key")),
            "{printed:?}"
        );
    }
}
