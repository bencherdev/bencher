use bencher_json::{RunnerResourceId, Secret, UpdateChannel};

use super::merge_ssh_with_extras;
use super::ssh::{Ssh, Stdin};
use crate::parser::TaskStart;

const DROP_IN_DIR: &str = "/etc/systemd/system/bencher-runner.service.d";
const KEY_DIR: &str = "/etc/bencher-runner";
const KEY_FILE: &str = "/etc/bencher-runner/key.env";

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
        let credentials = credentials_conf(&host, &runner, update_channel, danger_allow_no_sandbox);
        run_steps(
            start_steps(&key, &credentials),
            |command| ssh.run(command),
            |command, stdin| ssh.run_quiet_with_stdin(command, stdin),
        )?;
        println!("Runner is running");
        Ok(())
    }
}

/// Run each step through `printed`, or through `quiet` with its stdin when it must not be printed.
fn run_steps(
    steps: impl IntoIterator<Item = Step>,
    mut printed: impl FnMut(&str) -> anyhow::Result<String>,
    mut quiet: impl FnMut(&str, &Stdin) -> anyhow::Result<String>,
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
            Echo::Quiet { stdin } => quiet(&command, &stdin)?,
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
    /// Neither is printed, and `stdin` reaches the command without entering any process's command line.
    Quiet {
        stdin: Stdin,
    },
}

/// Writing the key file is the one quiet step, and if no key arrived it fails
/// before the old key file changes.
fn start_steps(key: &Secret, credentials: &str) -> [Step; 6] {
    [
        Step {
            notice: Some("Configuring runner credentials..."),
            command: format!("mkdir -p {DROP_IN_DIR} && install -d -m 700 {KEY_DIR}"),
            echo: Echo::Printed,
        },
        Step {
            notice: Some("Writing the runner key..."),
            command: format!(
                "{} && test -s {KEY_FILE}.new && mv -f {KEY_FILE}.new {KEY_FILE}",
                write_root_only(&format!("{KEY_FILE}.new"))
            ),
            echo: Echo::Quiet {
                stdin: key_env(key),
            },
        },
        Step {
            notice: Some("Writing runner credentials..."),
            command: format!(
                "{} << 'CRED_EOF'\n{credentials}CRED_EOF",
                write_root_only(&format!("{DROP_IN_DIR}/credentials.conf"))
            ),
            echo: Echo::Printed,
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

/// Run as root, `install` replaces any older file and its mode with its stdin, and the umask keeps the new file private from its creation.
fn write_root_only(path: &str) -> String {
    format!("umask 077 && install -m 600 /dev/stdin {path}")
}

/// The runner reads the key from its environment, loaded from a file `systemctl show` never lists.
fn key_env(key: &Secret) -> Stdin {
    Stdin(format!("BENCHER_RUNNER_KEY={}\n", key.as_ref()))
}

/// Build the contents of the systemd credentials drop-in.
fn credentials_conf(
    host: &url::Url,
    runner: &RunnerResourceId,
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
         EnvironmentFile={KEY_FILE}\n\
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
        let conf = credentials_conf(&test_host(), &test_runner(), None, false);
        assert_eq!(
            conf,
            "[Service]\n\
             Environment=BENCHER_HOST=https://api.example.com/\n\
             Environment=BENCHER_RUNNER=test-runner\n\
             EnvironmentFile=/etc/bencher-runner/key.env\n"
        );
    }

    #[test]
    fn credentials_conf_with_channel() {
        let conf = credentials_conf(
            &test_host(),
            &test_runner(),
            Some(UpdateChannel::Canary),
            false,
        );
        assert!(conf.contains("Environment=BENCHER_UPDATE_CHANNEL=canary\n"));
    }

    #[test]
    fn credentials_conf_with_no_sandbox() {
        let conf = credentials_conf(&test_host(), &test_runner(), None, true);
        assert!(conf.contains("Environment=BENCHER_DANGER_ALLOW_NO_SANDBOX=true\n"));
    }

    #[test]
    fn start_sends_the_key_on_stdin_alone() {
        let credentials = credentials_conf(
            &test_host(),
            &test_runner(),
            Some(UpdateChannel::Canary),
            true,
        );
        let mut printed = Vec::new();
        let mut quiet = Vec::new();
        let mut stdins = Vec::new();
        run_steps(
            start_steps(&"secret-key".parse().unwrap(), &credentials),
            |command| {
                printed.push(command.to_owned());
                Ok(String::new())
            },
            |command, stdin| {
                quiet.push(command.to_owned());
                stdins.push(stdin.0.clone());
                Ok(String::new())
            },
        )
        .unwrap();
        assert_eq!(stdins, ["BENCHER_RUNNER_KEY=secret-key\n"]);
        assert!(
            printed
                .iter()
                .chain(&quiet)
                .all(|command| !command.contains("secret-key")),
            "{printed:?} {quiet:?}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_root_only_replaces_a_readable_file() {
        use std::fs::Permissions;
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;
        use std::process::{Command, Stdio};

        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8Path::from_path(dir.path())
            .unwrap()
            .join("key.env");
        std::fs::write(&path, "BENCHER_RUNNER_KEY=old\n").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();

        let mut write = Command::new("sh")
            .arg("-c")
            .arg(write_root_only(path.as_str()))
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        write
            .stdin
            .take()
            .unwrap()
            .write_all(key_env(&"secret-key".parse().unwrap()).0.as_bytes())
            .unwrap();
        assert!(write.wait().unwrap().success());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "BENCHER_RUNNER_KEY=secret-key\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_key_lost_on_the_way_fails_the_start() {
        // Prevents a key lost on the way emptying the working key file or
        // reporting success, and a delivered key keeping an older file's mode.
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8Path::from_path(dir.path())
            .unwrap()
            .join("key.env");
        std::fs::write(&path, "BENCHER_RUNNER_KEY=old\n").unwrap();
        std::fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();

        assert!(
            !key_step_succeeds(&path, false),
            "an empty key file must fail the start"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "BENCHER_RUNNER_KEY=old\n",
            "a lost key must leave the old key file as it was"
        );
        assert!(key_step_succeeds(&path, true), "a delivered key is written");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "BENCHER_RUNNER_KEY=secret-key\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600,
            "a delivered key replaces an older file's mode"
        );
    }

    /// Runs the key step on the local host with its file at `path`, losing its stdin unless `delivered`.
    #[cfg(target_os = "linux")]
    fn key_step_succeeds(path: &camino::Utf8Path, delivered: bool) -> bool {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        run_steps(
            start_steps(&"secret-key".parse().unwrap(), ""),
            |_command| Ok(String::new()),
            |command, stdin| {
                let mut write = Command::new("sh")
                    .arg("-c")
                    .arg(command.replace(KEY_FILE, path.as_str()))
                    .stdin(Stdio::piped())
                    .spawn()?;
                let mut pipe = write.stdin.take().unwrap();
                if delivered {
                    pipe.write_all(stdin.0.as_bytes())?;
                }
                drop(pipe);
                anyhow::ensure!(write.wait()?.success(), "the key step failed");
                Ok(String::new())
            },
        )
        .is_ok()
    }
}
