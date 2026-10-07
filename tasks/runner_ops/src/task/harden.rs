use super::apt;
use super::ssh::Ssh;

// Ships marked automatic with no dependents, so autoremove would take it from some runners and not others.
const KERNEL_ACCESSORIES: &str = "ubuntu-kernel-accessories";

pub fn harden(ssh: &Ssh) -> anyhow::Result<()> {
    // Update system
    println!("Updating system packages...");
    ssh.run("apt-get update && DEBIAN_FRONTEND=noninteractive apt-get upgrade -y")?;

    // Install packages
    println!("Installing required packages...");
    ssh.run(
        "DEBIAN_FRONTEND=noninteractive apt-get install -y curl ufw fail2ban unattended-upgrades",
    )?;

    keep_kernel_accessories(ssh)?;
    warn_autoremovable(ssh)?;

    // Firewall
    println!("Configuring firewall...");
    ssh.run("ufw allow ssh && ufw --force enable")?;

    // fail2ban
    println!("Enabling fail2ban...");
    ssh.run("systemctl enable fail2ban && systemctl start fail2ban")?;

    println!("Server hardening complete");
    Ok(())
}

fn keep_kernel_accessories(ssh: &Ssh) -> anyhow::Result<()> {
    let installed = ssh.check(&format!(
        "dpkg-query -W -f='${{db:Status-Status}}' {KERNEL_ACCESSORIES} 2>/dev/null | grep -qx installed"
    ))?;
    if installed {
        println!("Marking {KERNEL_ACCESSORIES} as manually installed...");
        ssh.run(&format!("apt-mark manual {KERNEL_ACCESSORIES}"))?;
    }
    Ok(())
}

fn warn_autoremovable(ssh: &Ssh) -> anyhow::Result<()> {
    let simulation = ssh.run(apt::AUTOREMOVE_SIMULATION)?;
    let packages = apt::autoremovable_packages(&simulation);
    if !packages.is_empty() {
        println!(
            "Warning: auto-removable packages left behind, so this runner will drift from the fleet: {}",
            packages.join(" ")
        );
    }
    Ok(())
}
