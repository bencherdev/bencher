use super::desired::{self, Applied, Desired, Finding};
use super::download::published_checksum;
use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskHost;
use crate::parser::server::load_server;

#[derive(Debug)]
pub struct Host {
    label: String,
    ssh: Ssh,
    desired: Desired,
}

impl TryFrom<TaskHost> for Host {
    type Error = anyhow::Error;

    fn try_from(task: TaskHost) -> anyhow::Result<Self> {
        let TaskHost {
            runner,
            server,
            ssh,
            user,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let update_channel = file
            .as_ref()
            .and_then(|f| f.update_channel)
            .unwrap_or_default();
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        let label = runner.map_or_else(|| server.clone(), |runner| runner.to_string());
        Ok(Self {
            label,
            ssh: Ssh::new(server, ssh, user),
            desired: Desired::new(update_channel),
        })
    }
}

impl Host {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            label,
            ssh,
            desired,
        } = self;
        host(&ssh, &label, &desired)
    }
}

/// Write every host setting that differs, report the desired state, and fail if a setting `host` writes still differs.
pub fn host(ssh: &Ssh, label: &str, desired: &Desired) -> anyhow::Result<()> {
    println!("Applying the host settings to {label}...");
    let Applied { written, findings } = desired.apply(
        |script| ssh.run_quiet(script),
        |command| ssh.run(command),
        &mut published_checksum,
    )?;
    desired::report(label, &findings);
    if written == 0 {
        println!("Wrote no host settings to {label}, so nothing changed");
    } else {
        println!("Wrote {written} host setting(s) to {label}");
    }
    let unset: Vec<&str> = findings
        .iter()
        .filter(|finding| finding.host_unset())
        .map(Finding::name)
        .collect();
    anyhow::ensure!(
        unset.is_empty(),
        "Host settings still differ after writing them: {}",
        unset.join(", ")
    );
    Ok(())
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use bencher_json::UpdateChannel;
    use camino::Utf8Path;

    use super::*;
    use crate::task::desired::{BENCHER_CGROUP, SSH_HARDENING_PATH};
    use crate::task::start::KEY_FILE;

    #[test]
    fn host_fails_when_a_setting_it_writes_still_differs() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        // A runner on which no write takes, since every read prints nothing.
        let ssh = Ssh::stand_in(dir, "#!/bin/sh\n").unwrap();
        let error = host(&ssh, "runner", &Desired::new(UpdateChannel::Stable))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("Host settings still differ after writing them: "),
            "{error}"
        );
        assert!(error.contains(SSH_HARDENING_PATH), "{error}");
        for set_elsewhere in [KEY_FILE, BENCHER_CGROUP] {
            assert!(!error.contains(set_elsewhere), "{error}");
        }
    }
}
