use std::thread;
use std::time::Duration;

use super::ssh::Ssh;

const AUTOSETUP_CONFIG: &str = "\
DRIVE1 /dev/nvme0n1
DRIVE2 /dev/nvme1n1
SWRAID 1
SWRAIDLEVEL 1
BOOTLOADER grub
HOSTNAME bencher
PART /boot ext4 1G
PART swap swap 4G
PART / ext4 all
IMAGE /root/.oldroot/nfs/images/Ubuntu-2404-noble-amd64-base.tar.gz";

const REBOOT_POLL_INTERVAL: Duration = Duration::from_secs(10);
const REBOOT_TIMEOUT: Duration = Duration::from_secs(300);

pub fn install_os(ssh: &Ssh) -> anyhow::Result<()> {
    // Detect if we're in rescue mode
    let in_rescue = ssh.check("test -d /root/.oldroot/nfs")?;
    if !in_rescue {
        println!("OS already installed, skipping install_os");
        return Ok(());
    }

    println!("Rescue mode detected, installing OS...");

    // Write autosetup config and run installimage
    ssh.run(&format!(
        "cat > /tmp/autosetup << 'AUTOSETUP_EOF'\n{AUTOSETUP_CONFIG}\nAUTOSETUP_EOF"
    ))?;
    ssh.run("/root/.oldroot/nfs/install/installimage -a -c /tmp/autosetup")?;

    // Reboot (will disconnect — ignore connection error)
    println!("Rebooting server...");
    let _ignored = ssh.run("reboot");

    // Host key will change after reinstall.
    // Remove known_hosts AFTER reboot so we also clear any entry
    // re-added by the reboot SSH connection via accept-new.
    ssh.remove_known_host()?;

    // Wait for SSH to come back up
    wait_for_ssh(ssh)?;

    Ok(())
}

fn wait_for_ssh(ssh: &Ssh) -> anyhow::Result<()> {
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

        match ssh.check("true") {
            Ok(true) => {
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
