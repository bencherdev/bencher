use anyhow::Context as _;

use super::merge_ssh;
use super::ssh::Ssh;
use super::stop::stop_service;
use crate::parser::TaskIsolate;
use crate::parser::server::load_server;

// The `zz-` prefix sorts this drop-in after provider drop-ins (e.g. Hetzner's
// `hetzner.cfg`) that overwrite GRUB_CMDLINE_LINUX_DEFAULT instead of
// appending; /etc/default/grub.d/*.cfg files are sourced in glob order.
const GRUB_DROP_IN: &str = "/etc/default/grub.d/zz-bencher-isolation.cfg";

#[derive(Debug)]
pub struct Isolate {
    ssh: Ssh,
    cpus: Option<String>,
}

impl TryFrom<TaskIsolate> for Isolate {
    type Error = anyhow::Error;

    fn try_from(task: TaskIsolate) -> anyhow::Result<Self> {
        let TaskIsolate {
            runner,
            server,
            ssh,
            user,
            cpus,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        Ok(Self {
            ssh: Ssh::new(server, ssh, user),
            cpus,
        })
    }
}

impl Isolate {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { ssh, cpus } = self;

        let cmdline = ssh.run("cat /proc/cmdline")?;
        if is_isolated(&cmdline) {
            println!("CPU isolation boot args already configured");
            return Ok(());
        }

        let nproc = ssh
            .run("nproc")?
            .trim()
            .parse()
            .context("failed to parse `nproc` output")?;
        let cpus = if let Some(cpus) = cpus {
            cpus
        } else {
            benchmark_cpus(nproc)?
        };
        validate_cpu_list(&cpus, nproc)?;

        println!("Configuring CPU isolation boot args for CPUs {cpus}...");
        let cfg = isolation_cfg(&cpus);
        ssh.run(&format!(
            "mkdir -p /etc/default/grub.d && cat > {GRUB_DROP_IN} << 'GRUB_EOF'\n{cfg}\nGRUB_EOF"
        ))?;
        ssh.run("update-grub")?;
        if !ssh.check(&format!("grep -qF 'isolcpus={cpus}' /boot/grub/grub.cfg"))? {
            anyhow::bail!(
                "generated GRUB config is missing the isolation args; another /etc/default/grub.d drop-in may be overwriting GRUB_CMDLINE_LINUX_DEFAULT"
            );
        }

        let boot_id = ssh.boot_id()?;
        stop_service(&ssh)?;

        // Reboot (will disconnect; ignore connection error)
        println!("Rebooting server...");
        let _ignored = ssh.run("reboot");
        ssh.wait_for_reboot(&boot_id)?;

        let cmdline = ssh.run("cat /proc/cmdline")?;
        if !is_isolated(&cmdline) {
            anyhow::bail!("CPU isolation boot args did not take effect: {cmdline}");
        }
        println!("CPU isolation boot args are active");

        if ssh.check("systemctl is-active --quiet bencher-runner")? {
            println!("Runner is running");
        } else {
            println!(
                "Runner service is not active yet; check `cargo ops logs` or restart it with `cargo ops start`"
            );
        }
        Ok(())
    }
}

/// Whether the kernel cmdline already has CPU isolation boot args.
/// Mirrors the runner preflight check: either arg counts as isolation.
fn is_isolated(cmdline: &str) -> bool {
    cmdline.contains("isolcpus=") || cmdline.contains("nohz_full=")
}

/// Benchmark CPU list: CPU 0 is housekeeping, the rest run benchmarks.
/// Assumes contiguous logical CPU numbering; pass `--cpus` explicitly for
/// non-trivial topologies (e.g. SMT sibling layouts).
fn benchmark_cpus(nproc: u32) -> anyhow::Result<String> {
    match nproc {
        0 | 1 => anyhow::bail!("CPU isolation requires at least 2 CPUs, found {nproc}"),
        2 => Ok("1".to_owned()),
        _ => Ok(format!("1-{}", nproc - 1)),
    }
}

/// Validate a kernel CPU list (comma-separated CPUs or ascending `a-b` ranges)
/// so typos fail fast instead of after a reboot cycle.
/// CPU 0 is reserved for housekeeping and values must be below `nproc`.
fn validate_cpu_list(cpus: &str, nproc: u32) -> anyhow::Result<()> {
    let parse_cpu = |cpu: &str| {
        cpu.parse::<u32>()
            .ok()
            .filter(|cpu| (1..nproc).contains(cpu))
    };
    let valid = !cpus.is_empty()
        && cpus.split(',').all(|part| {
            let mut bounds = part.split('-');
            match (bounds.next(), bounds.next(), bounds.next()) {
                (Some(cpu), None, None) => parse_cpu(cpu).is_some(),
                (Some(start), Some(end), None) => matches!(
                    (parse_cpu(start), parse_cpu(end)),
                    (Some(start), Some(end)) if start <= end
                ),
                _ => false,
            }
        });
    if valid {
        Ok(())
    } else {
        anyhow::bail!("invalid CPU list for {nproc} CPUs: {cpus}")
    }
}

/// Build the contents of the GRUB drop-in, appending to the existing cmdline.
fn isolation_cfg(cpus: &str) -> String {
    format!(
        "GRUB_CMDLINE_LINUX_DEFAULT=\"$GRUB_CMDLINE_LINUX_DEFAULT isolcpus={cpus} nohz_full={cpus} rcu_nocbs={cpus}\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_isolated_detects_either_arg() {
        assert!(is_isolated("ro isolcpus=1-5 quiet"));
        assert!(is_isolated("ro nohz_full=1-5 quiet"));
        assert!(!is_isolated(
            "BOOT_IMAGE=/vmlinuz-6.8.0-90-generic ro consoleblank=0"
        ));
    }

    #[test]
    fn benchmark_cpus_range() {
        assert_eq!(benchmark_cpus(6).unwrap(), "1-5");
        assert_eq!(benchmark_cpus(12).unwrap(), "1-11");
    }

    #[test]
    fn benchmark_cpus_single() {
        assert_eq!(benchmark_cpus(2).unwrap(), "1");
    }

    #[test]
    fn benchmark_cpus_too_few() {
        benchmark_cpus(0).unwrap_err();
        benchmark_cpus(1).unwrap_err();
    }

    #[test]
    fn validate_cpu_list_ok() {
        validate_cpu_list("1", 2).unwrap();
        validate_cpu_list("1-5", 6).unwrap();
        validate_cpu_list("1,3,5", 6).unwrap();
        validate_cpu_list("1-5,7", 8).unwrap();
    }

    #[test]
    fn validate_cpu_list_invalid() {
        validate_cpu_list("", 6).unwrap_err();
        validate_cpu_list("1-", 6).unwrap_err();
        validate_cpu_list(",1", 6).unwrap_err();
        validate_cpu_list("1--5", 6).unwrap_err();
        validate_cpu_list("1 5", 6).unwrap_err();
        validate_cpu_list("garbage", 6).unwrap_err();
        validate_cpu_list("1-2-3", 6).unwrap_err();
        validate_cpu_list("5-1", 6).unwrap_err();
    }

    #[test]
    fn validate_cpu_list_out_of_bounds() {
        // CPU 0 is the housekeeping core
        validate_cpu_list("0-5", 6).unwrap_err();
        // Values must be below nproc
        validate_cpu_list("1-6", 6).unwrap_err();
        validate_cpu_list("1-9999", 6).unwrap_err();
    }

    #[test]
    fn isolation_cfg_appends_all_args() {
        let cfg = isolation_cfg("1-5");
        assert_eq!(
            cfg,
            "GRUB_CMDLINE_LINUX_DEFAULT=\"$GRUB_CMDLINE_LINUX_DEFAULT isolcpus=1-5 nohz_full=1-5 rcu_nocbs=1-5\""
        );
    }
}
