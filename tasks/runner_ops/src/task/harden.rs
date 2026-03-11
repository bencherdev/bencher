use super::ssh::Ssh;

const SSH_HARDENING_CONF: &str = "\
PasswordAuthentication no
ChallengeResponseAuthentication no
KbdInteractiveAuthentication no
X11Forwarding no
PermitRootLogin prohibit-password";

const UNATTENDED_UPGRADES_CONF: &str = r#"\
Unattended-Upgrade::Allowed-Origins {
    "${distro_id}:${distro_codename}-security";
};
Unattended-Upgrade::AutoFixInterruptedDpkg "true";
Unattended-Upgrade::Remove-Unused-Kernel-Packages "true";
Unattended-Upgrade::Remove-Unused-Dependencies "true";"#;

const AUTO_UPGRADES_CONF: &str = "\
APT::Periodic::Update-Package-Lists \"1\";
APT::Periodic::Unattended-Upgrade \"1\";";

pub fn harden(ssh: &Ssh) -> anyhow::Result<()> {
    // Update system
    println!("Updating system packages...");
    ssh.run("apt-get update && DEBIAN_FRONTEND=noninteractive apt-get upgrade -y")?;

    // Install packages
    println!("Installing required packages...");
    ssh.run(
        "DEBIAN_FRONTEND=noninteractive apt-get install -y curl ufw fail2ban unattended-upgrades",
    )?;

    // Harden SSH
    println!("Hardening SSH configuration...");
    ssh.run(&format!(
        "cat > /etc/ssh/sshd_config.d/hardening.conf << 'SSHD_EOF'\n{SSH_HARDENING_CONF}\nSSHD_EOF"
    ))?;
    ssh.run("systemctl reload ssh")?;

    // Firewall
    println!("Configuring firewall...");
    ssh.run("ufw allow ssh && ufw --force enable")?;

    // fail2ban
    println!("Enabling fail2ban...");
    ssh.run("systemctl enable fail2ban && systemctl start fail2ban")?;

    // Unattended upgrades
    println!("Configuring unattended upgrades...");
    ssh.run(&format!(
        "cat > /etc/apt/apt.conf.d/50unattended-upgrades-local << 'UU_EOF'\n{UNATTENDED_UPGRADES_CONF}\nUU_EOF"
    ))?;
    ssh.run(&format!(
        "cat > /etc/apt/apt.conf.d/20auto-upgrades << 'AU_EOF'\n{AUTO_UPGRADES_CONF}\nAU_EOF"
    ))?;

    println!("Server hardening complete");
    Ok(())
}
