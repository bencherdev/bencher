use super::ssh::Ssh;

const IMAGE: &str = "/root/.oldroot/nfs/images/Ubuntu-2404-noble-amd64-base.tar.zst";
const INSTALLIMAGE: &str = "/root/.oldroot/nfs/install/installimage";
const INSTALLIMAGE_LOG: &str = "/root/debug.txt";
const INSTALLIMAGE_LOG_LINES: u32 = 64;

pub fn install_os(ssh: &Ssh) -> anyhow::Result<()> {
    // Detect if we're in rescue mode
    let in_rescue = ssh.check("test -d /root/.oldroot/nfs")?;
    if !in_rescue {
        println!("OS already installed, skipping install_os");
        return Ok(());
    }

    println!("Rescue mode detected, installing OS...");

    if !ssh.check(&format!("test -f {IMAGE}"))? {
        anyhow::bail!("OS image not found on the rescue system: {IMAGE}");
    }

    // Write autosetup config and run installimage
    let autosetup = autosetup_config();
    ssh.run(&format!(
        "cat > /tmp/autosetup << 'AUTOSETUP_EOF'\n{autosetup}\nAUTOSETUP_EOF"
    ))?;
    // Without a terminal type, installimage's `clear` buries the real error in noise.
    if let Err(error) = ssh.run(&format!("TERM=xterm {INSTALLIMAGE} -a -c /tmp/autosetup")) {
        let log = ssh
            .run_quiet(&format!(
                "tail -n {INSTALLIMAGE_LOG_LINES} {INSTALLIMAGE_LOG}"
            ))
            .unwrap_or_else(|log_error| format!("(could not read the log: {log_error})"));
        return Err(error.context(format!(
            "installimage failed; tail of {INSTALLIMAGE_LOG}:\n{log}"
        )));
    }

    // Capture the rescue system's boot ID to detect the post-install boot
    let boot_id = ssh.boot_id()?;

    // Reboot (will disconnect; ignore connection error)
    println!("Rebooting server...");
    let _ignored = ssh.run("reboot");

    // Host key will change after reinstall.
    // Remove known_hosts AFTER reboot so we also clear any entry
    // re-added by the reboot SSH connection via accept-new.
    ssh.remove_known_host()?;

    // Wait for SSH to come back up
    ssh.wait_for_reboot(&boot_id)?;

    Ok(())
}

fn autosetup_config() -> String {
    format!(
        "\
DRIVE1 /dev/nvme0n1
DRIVE2 /dev/nvme1n1
SWRAID 1
SWRAIDLEVEL 1
BOOTLOADER grub
HOSTNAME bencher
PART /boot ext4 1G
PART swap swap 4G
PART / ext4 all
IMAGE {IMAGE}"
    )
}
