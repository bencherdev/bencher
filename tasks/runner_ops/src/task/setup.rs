use camino::Utf8Path;

use super::ssh::Ssh;
use super::stop::stop_service;

const SYSTEMD_SERVICE: &str = "\
[Unit]
Description=Bencher Runner
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/runner up
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target";

pub fn deploy(ssh: &Ssh, runner_binary: Option<&Utf8Path>) -> anyhow::Result<()> {
    // System verification
    println!("Verifying system capabilities...");

    let has_kvm = ssh.check("test -c /dev/kvm")?;
    if !has_kvm {
        anyhow::bail!("/dev/kvm not found — KVM is required");
    }
    println!("  KVM: available");

    let has_cgroups_v2 = ssh.check("test -f /sys/fs/cgroup/cgroup.controllers")?;
    if !has_cgroups_v2 {
        anyhow::bail!("cgroups v2 not available — required for runner");
    }
    println!("  cgroups v2: available");

    let controllers = ssh.run("cat /sys/fs/cgroup/cgroup.controllers")?;
    let controllers = controllers.trim();
    for required in ["cpu", "memory", "pids"] {
        if !controllers.split_whitespace().any(|c| c == required) {
            anyhow::bail!("Required cgroup controller '{required}' not found in: {controllers}");
        }
    }
    println!("  cgroup controllers: {controllers}");

    // Set timezone to UTC
    ssh.run("timedatectl set-timezone UTC")?;
    println!("  timezone: UTC");

    // Deploy runner (only if binary provided)
    let Some(runner_binary) = runner_binary else {
        println!("No --runner-binary provided, skipping deployment");
        return Ok(());
    };

    stop_service(ssh)?;

    println!("Deploying runner binary...");
    ssh.copy_to(runner_binary, "/usr/local/bin/runner")?;
    ssh.run("chmod +x /usr/local/bin/runner")?;

    // Write systemd service
    ssh.run(&format!(
        "cat > /etc/systemd/system/bencher-runner.service << 'SVC_EOF'\n{SYSTEMD_SERVICE}\nSVC_EOF"
    ))?;
    ssh.run("systemctl daemon-reload && systemctl enable bencher-runner")?;

    println!("Runner deployed successfully");

    Ok(())
}
