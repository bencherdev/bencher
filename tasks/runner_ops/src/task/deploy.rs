use camino::Utf8Path;

use super::ssh::Ssh;

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
Environment=BENCHER_HOST=https://api.bencher.dev

[Install]
WantedBy=multi-user.target";

pub fn deploy(ssh: &Ssh, runner_binary: Option<&Utf8Path>) -> anyhow::Result<()> {
    // Step 9: System verification
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

    // Step 10: Deploy runner (only if binary provided)
    let Some(runner_binary) = runner_binary else {
        println!("No --runner-binary provided, skipping deployment");
        return Ok(());
    };

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

pub fn start(ssh: &Ssh, runner: &str, token: &str) -> anyhow::Result<()> {
    println!("Configuring runner credentials...");
    ssh.run("mkdir -p /etc/systemd/system/bencher-runner.service.d")?;
    ssh.run(&format!(
        "cat > /etc/systemd/system/bencher-runner.service.d/credentials.conf << 'CRED_EOF'\n\
         [Service]\n\
         Environment=BENCHER_RUNNER={runner}\n\
         Environment=BENCHER_RUNNER_TOKEN={token}\n\
         CRED_EOF"
    ))?;

    println!("Starting runner service...");
    ssh.run("systemctl daemon-reload")?;
    ssh.run("systemctl restart bencher-runner")?;
    ssh.run("systemctl status bencher-runner")?;

    println!("Runner is running");
    Ok(())
}
