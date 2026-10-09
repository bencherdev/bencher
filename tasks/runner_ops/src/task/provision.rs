use camino::Utf8PathBuf;

use super::deploy_setup;
use super::desired::Desired;
use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskProvision;
use crate::parser::server::load_server;

#[derive(Debug)]
pub struct Provision {
    label: String,
    ssh: Ssh,
    desired: Desired,
    runner_binary: Option<Utf8PathBuf>,
}

impl TryFrom<TaskProvision> for Provision {
    type Error = anyhow::Error;

    fn try_from(task: TaskProvision) -> anyhow::Result<Self> {
        let TaskProvision {
            runner,
            server,
            ssh,
            user,
            runner_binary,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let update_channel = file
            .as_ref()
            .and_then(|f| f.update_channel)
            .unwrap_or_default();
        let scrub_day = file.as_ref().and_then(|f| f.scrub_day);
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        let label = runner.map_or_else(|| server.clone(), |runner| runner.to_string());
        Ok(Self {
            label,
            ssh: Ssh::new(server, ssh, user),
            desired: Desired::new(update_channel, scrub_day),
            runner_binary,
        })
    }
}

impl Provision {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            label,
            ssh,
            desired,
            runner_binary,
        } = self;
        super::install_os::install_os(&ssh)?;
        // Before the first upgrade, so the kernel pin governs it.
        super::host::host(&ssh, &label, &desired)?;
        super::harden::harden(&ssh)?;
        deploy_setup::deploy(&ssh, runner_binary.as_deref())?;
        Ok(())
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use std::fs;

    use bencher_json::UpdateChannel;
    use camino::Utf8Path;

    use super::*;
    use crate::task::desired::KERNEL_PIN_PATH;

    #[test]
    fn provision_applies_the_host_settings_before_the_first_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        // A runner out of rescue that records each command, on which no host write takes and every upgrade fails.
        let ssh = Ssh::stand_in(
            dir,
            r#"#!/bin/sh
for command; do :; done
printf '%s\n@@end\n' "$command" >> "$0.log"
case "$command" in
  "test -d /root/.oldroot/nfs" | *apt-get*) exit 1 ;;
esac
"#,
        )
        .unwrap();
        let error = Provision {
            label: "runner".to_owned(),
            ssh,
            desired: Desired::new(UpdateChannel::Stable, None),
            runner_binary: None,
        }
        .exec()
        .unwrap_err()
        .to_string();
        let log = fs::read_to_string(dir.join("ssh.log")).unwrap();
        let commands: Vec<&str> = log.split_terminator("\n@@end\n").collect();
        assert!(
            error.starts_with("Host settings still differ after writing them: "),
            "{error}"
        );
        assert_eq!(
            commands.first(),
            Some(&"test -d /root/.oldroot/nfs"),
            "{commands:#?}"
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains(KERNEL_PIN_PATH)),
            "{commands:#?}"
        );
        assert!(
            !commands.iter().any(|command| command.contains("apt-get")),
            "{commands:#?}"
        );
    }
}
