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
    // Step 3: Update system
    println!("Updating system packages...");
    ssh.run("apt-get update && DEBIAN_FRONTEND=noninteractive apt-get upgrade -y")?;

    // Step 4: Install packages
    println!("Installing required packages...");
    ssh.run(
        "DEBIAN_FRONTEND=noninteractive apt-get install -y curl ufw fail2ban unattended-upgrades",
    )?;

    // Step 5: Harden SSH
    println!("Hardening SSH configuration...");
    ssh.run(&format!(
        "cat > /etc/ssh/sshd_config.d/hardening.conf << 'SSHD_EOF'\n{SSH_HARDENING_CONF}\nSSHD_EOF"
    ))?;
    ssh.run("systemctl reload ssh")?;

    // Step 6: Firewall
    println!("Configuring firewall...");
    ssh.run("ufw allow ssh && ufw --force enable")?;

    // Step 7: fail2ban
    println!("Enabling fail2ban...");
    ssh.run("systemctl enable fail2ban && systemctl start fail2ban")?;

    // Step 8: Unattended upgrades
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
