use std::process::Command;
use std::thread;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};

const REBOOT_POLL_INTERVAL: Duration = Duration::from_secs(10);
const REBOOT_TIMEOUT: Duration = Duration::from_mins(5);
const BOOT_ID_CMD: &str = "cat /proc/sys/kernel/random/boot_id";

#[derive(Debug)]
pub struct Ssh {
    server: String,
    key: Utf8PathBuf,
    user: String,
}

impl Ssh {
    pub fn new(server: String, key: Utf8PathBuf, user: String) -> Self {
        Self { server, key, user }
    }

    fn destination(&self) -> String {
        format!("{}@{}", self.user, self.server)
    }

    fn ssh_options(&self) -> Vec<String> {
        vec![
            "-o".into(),
            "ConnectTimeout=10".into(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-i".into(),
            self.key.to_string(),
        ]
    }

    /// Run a command on the remote server, returning stdout.
    pub fn run(&self, command: &str) -> anyhow::Result<String> {
        println!("ssh: {command}");
        let output = Command::new("ssh")
            .args(self.ssh_options())
            .arg(self.destination())
            .arg(command)
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if !stdout.is_empty() {
                println!("{stdout}");
            }
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "SSH command failed (exit {}): {stderr}",
                output.status.code().unwrap_or(-1)
            );
        }
    }

    /// Run a command, returning Ok(true) if exit code 0, Ok(false) if non-zero.
    /// Does not treat non-zero exit as an error (for idempotent checks).
    pub fn check(&self, command: &str) -> anyhow::Result<bool> {
        println!("ssh check: {command}");
        let status = Command::new("ssh")
            .args(self.ssh_options())
            .arg(self.destination())
            .arg(command)
            .status()?;
        Ok(status.success())
    }

    /// Copy a local file to the remote server.
    pub fn copy_to(&self, local: &Utf8Path, remote: &str) -> anyhow::Result<()> {
        println!("scp: {local} -> {remote}");
        let status = Command::new("scp")
            .args(self.ssh_options())
            .arg(local.as_str())
            .arg(format!("{}:{remote}", self.destination()))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("SCP failed (exit {})", status.code().unwrap_or(-1));
        }
    }

    /// Run a command on the remote server with inherited stdio (streams directly to terminal).
    pub fn exec(&self, command: &str) -> anyhow::Result<()> {
        let status = Command::new("ssh")
            .args(self.ssh_options())
            .arg(self.destination())
            .arg(command)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("SSH command failed (exit {})", status.code().unwrap_or(-1));
        }
    }

    /// Read the kernel boot ID, used to detect when a reboot has completed.
    pub fn boot_id(&self) -> anyhow::Result<String> {
        Ok(self.run(BOOT_ID_CMD)?.trim().to_owned())
    }

    /// Wait for the server to come back online after a reboot.
    /// A probe only counts once the boot ID differs from `old_boot_id`,
    /// so a not-yet-rebooted system cannot satisfy the wait.
    pub fn wait_for_reboot(&self, old_boot_id: &str) -> anyhow::Result<()> {
        println!("Waiting for server to come back online...");
        let mut elapsed = Duration::ZERO;
        loop {
            thread::sleep(REBOOT_POLL_INTERVAL);
            elapsed += REBOOT_POLL_INTERVAL;

            if elapsed > REBOOT_TIMEOUT {
                anyhow::bail!(
                    "Server did not come back online within {} seconds",
                    REBOOT_TIMEOUT.as_secs()
                );
            }

            match self.run(BOOT_ID_CMD) {
                Ok(boot_id) if boot_id.trim() != old_boot_id => {
                    println!("Server is back online");
                    return Ok(());
                },
                _ => {
                    println!(
                        "Still waiting... ({}/{}s)",
                        elapsed.as_secs(),
                        REBOOT_TIMEOUT.as_secs()
                    );
                },
            }
        }
    }

    /// Remove the `known_hosts` entry for this host (used after OS reinstall changes host key).
    pub fn remove_known_host(&self) -> anyhow::Result<()> {
        println!("Removing known_hosts entry for {}", self.server);
        let status = Command::new("ssh-keygen")
            .arg("-R")
            .arg(&self.server)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "ssh-keygen -R failed (exit {})",
                status.code().unwrap_or(-1)
            );
        }
    }
}
