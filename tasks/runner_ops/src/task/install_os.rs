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
