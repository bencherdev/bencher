use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use super::merge_ssh;
use super::ssh::Ssh;
use super::stop::stop_service;
use crate::parser::TaskIsolate;
use crate::parser::server::load_server;

// The `zz-` prefix sorts this drop-in after provider drop-ins (e.g. Hetzner's
// `hetzner.cfg`) that overwrite GRUB_CMDLINE_LINUX_DEFAULT instead of
// appending; /etc/default/grub.d/*.cfg files are sourced in glob order.
const GRUB_DROP_IN: &str = "/etc/default/grub.d/zz-bencher-isolation.cfg";
// Offline CPUs have no topology directory, so the sibling lists follow the current SMT state.
const TOPOLOGY_CMD: &str = "cat /sys/devices/system/cpu/present /sys/devices/system/cpu/cpu[0-9]*/topology/thread_siblings_list";

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

        let topology = Topology::parse(&ssh.run(TOPOLOGY_CMD)?)?;
        let cpus = format_cpu_list(&if let Some(cpus) = cpus {
            validate_cpu_list(&cpus, &topology.present)?
        } else {
            topology.benchmark_cpus()?
        });

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
        } else if ssh.check("test -f /etc/systemd/system/bencher-runner.service")? {
            println!(
                "Runner service is not active yet; check `cargo ops logs` or restart it with `cargo ops start`"
            );
        } else {
            println!("Runner service is not installed yet; install it with `cargo ops deploy`");
        }
        Ok(())
    }
}

/// Whether the kernel cmdline already has CPU isolation boot args.
/// Mirrors the runner preflight check: either arg counts as isolation.
fn is_isolated(cmdline: &str) -> bool {
    cmdline.contains("isolcpus=") || cmdline.contains("nohz_full=")
}

/// The logical CPUs that exist and the SMT sibling groups of the online ones.
#[derive(Debug)]
struct Topology {
    present: BTreeSet<u32>,
    siblings: Vec<BTreeSet<u32>>,
}

impl Topology {
    /// Parse [`TOPOLOGY_CMD`] output: the present CPU list, then one sibling list per online CPU.
    fn parse(output: &str) -> anyhow::Result<Self> {
        let mut lists = output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty());
        let present = lists
            .next()
            .and_then(parse_cpu_list)
            .ok_or_else(|| anyhow::anyhow!("invalid CPU topology: {output}"))?;
        let siblings = lists
            .map(|list| {
                parse_cpu_list(list)
                    .ok_or_else(|| anyhow::anyhow!("invalid CPU topology: {output}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self { present, siblings })
    }

    /// The lowest logical CPU of each physical core, leaving the core with CPU 0 for housekeeping.
    fn benchmark_cpus(&self) -> anyhow::Result<BTreeSet<u32>> {
        let cpus: BTreeSet<u32> = self
            .siblings
            .iter()
            .filter_map(|siblings| siblings.first().copied())
            .filter(|&cpu| cpu != 0)
            .collect();
        if cpus.is_empty() {
            anyhow::bail!("CPU isolation requires at least 2 physical cores");
        }
        Ok(cpus)
    }
}

/// Validate a kernel CPU list (comma-separated CPUs or ascending `a-b` ranges)
/// so typos fail fast instead of after a reboot cycle.
/// CPU 0 is reserved for housekeeping and every CPU must exist.
fn validate_cpu_list(cpus: &str, present: &BTreeSet<u32>) -> anyhow::Result<BTreeSet<u32>> {
    let valid = parse_cpu_ranges(cpus).filter(|ranges| {
        ranges
            .iter()
            .cloned()
            .flatten()
            .all(|cpu| cpu != 0 && present.contains(&cpu))
    });
    if let Some(ranges) = valid {
        Ok(ranges.into_iter().flatten().collect())
    } else {
        anyhow::bail!(
            "invalid CPU list {cpus}: CPUs must exist ({}) and exclude housekeeping CPU 0",
            format_cpu_list(present)
        )
    }
}

fn parse_cpu_list(list: &str) -> Option<BTreeSet<u32>> {
    parse_cpu_ranges(list).map(|ranges| ranges.into_iter().flatten().collect())
}

fn parse_cpu_ranges(list: &str) -> Option<Vec<RangeInclusive<u32>>> {
    list.split(',')
        .map(|part| {
            let mut bounds = part.split('-');
            match (bounds.next(), bounds.next(), bounds.next()) {
                (Some(cpu), None, None) => parse_cpu(cpu).map(|cpu| cpu..=cpu),
                (Some(start), Some(end), None) => {
                    let (start, end) = (parse_cpu(start)?, parse_cpu(end)?);
                    (start <= end).then_some(start..=end)
                },
                _ => None,
            }
        })
        .collect()
}

/// The kernel rejects signs and spaces that `u32::from_str` would accept.
fn parse_cpu(cpu: &str) -> Option<u32> {
    if !cpu.is_empty() && cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        cpu.parse().ok()
    } else {
        None
    }
}

/// Format CPUs as a compact kernel CPU list (`1-5`, `2,4,6`).
fn format_cpu_list(cpus: &BTreeSet<u32>) -> String {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &cpu in cpus {
        match ranges.last_mut() {
            Some((_, end)) if end.checked_add(1) == Some(cpu) => *end = cpu,
            _ => ranges.push((cpu, cpu)),
        }
    }
    ranges
        .into_iter()
        .map(|(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
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

    // Intel numbering: core N has threads N and N+6.
    const INTEL_SMT_ON: &str =
        "0-11\n0,6\n1,7\n2,8\n3,9\n4,10\n5,11\n0,6\n1,7\n2,8\n3,9\n4,10\n5,11\n";
    const INTEL_SMT_OFF: &str = "0-11\n0\n1\n2\n3\n4\n5\n";
    // Interleaved numbering: core N has threads 2N and 2N+1.
    const AMD_SMT_ON: &str =
        "0-11\n0-1\n0-1\n2-3\n2-3\n4-5\n4-5\n6-7\n6-7\n8-9\n8-9\n10-11\n10-11\n";
    const AMD_SMT_OFF: &str = "0-11\n0\n2\n4\n6\n8\n10\n";

    fn benchmark_list(topology: &str) -> String {
        format_cpu_list(&Topology::parse(topology).unwrap().benchmark_cpus().unwrap())
    }

    fn cpus(range: RangeInclusive<u32>) -> BTreeSet<u32> {
        range.collect()
    }

    #[test]
    fn benchmark_cpus_intel_siblings() {
        assert_eq!(benchmark_list(INTEL_SMT_ON), "1-5");
        assert_eq!(benchmark_list(INTEL_SMT_OFF), "1-5");
    }

    #[test]
    fn benchmark_cpus_interleaved_siblings() {
        assert_eq!(benchmark_list(AMD_SMT_ON), "2,4,6,8,10");
        assert_eq!(benchmark_list(AMD_SMT_OFF), "2,4,6,8,10");
    }

    #[test]
    fn benchmark_cpus_without_smt() {
        assert_eq!(benchmark_list("0-1\n0\n1\n"), "1");
    }

    #[test]
    fn benchmark_cpus_too_few_cores() {
        let single_core = Topology::parse("0-1\n0-1\n0-1\n").unwrap();
        single_core.benchmark_cpus().unwrap_err();
    }

    #[test]
    fn topology_present_cpus() {
        let topology = Topology::parse(INTEL_SMT_OFF).unwrap();
        assert_eq!(topology.present, cpus(0..=11));
    }

    #[test]
    fn topology_invalid() {
        Topology::parse("").unwrap_err();
        Topology::parse("0-11\ngarbage\n").unwrap_err();
        Topology::parse("11-0\n0\n").unwrap_err();
    }

    #[test]
    fn format_cpu_list_compact() {
        assert_eq!(format_cpu_list(&cpus(1..=5)), "1-5");
        assert_eq!(format_cpu_list(&[2, 4, 6].into()), "2,4,6");
        assert_eq!(format_cpu_list(&[1, 2, 3, 5, 7, 8].into()), "1-3,5,7-8");
        assert_eq!(format_cpu_list(&[1].into()), "1");
    }

    #[test]
    fn validate_cpu_list_ok() {
        validate_cpu_list("1", &cpus(0..=1)).unwrap();
        validate_cpu_list("1-5", &cpus(0..=5)).unwrap();
        validate_cpu_list("1,3,5", &cpus(0..=5)).unwrap();
        validate_cpu_list("1-5,7", &cpus(0..=7)).unwrap();
        validate_cpu_list("2,4,6", &[0, 2, 4, 6].into()).unwrap();
    }

    #[test]
    fn validate_cpu_list_canonical() {
        let cpus = validate_cpu_list("5,1-3,2", &cpus(0..=5)).unwrap();
        assert_eq!(format_cpu_list(&cpus), "1-3,5");
    }

    #[test]
    fn validate_cpu_list_beyond_online_count() {
        // With SMT off only half the CPUs are online, but all of them exist.
        let topology = Topology::parse(INTEL_SMT_OFF).unwrap();
        validate_cpu_list("1-11", &topology.present).unwrap();
    }

    #[test]
    fn validate_cpu_list_invalid() {
        let present = cpus(0..=5);
        validate_cpu_list("", &present).unwrap_err();
        validate_cpu_list("1-", &present).unwrap_err();
        validate_cpu_list(",1", &present).unwrap_err();
        validate_cpu_list("1--5", &present).unwrap_err();
        validate_cpu_list("1 5", &present).unwrap_err();
        validate_cpu_list("garbage", &present).unwrap_err();
        validate_cpu_list("1-2-3", &present).unwrap_err();
        validate_cpu_list("5-1", &present).unwrap_err();
        validate_cpu_list("+1", &present).unwrap_err();
        validate_cpu_list("1-+5", &present).unwrap_err();
    }

    #[test]
    fn validate_cpu_list_out_of_bounds() {
        let present = cpus(0..=5);
        // CPU 0 is the housekeeping core
        validate_cpu_list("0-5", &present).unwrap_err();
        validate_cpu_list("1-6", &present).unwrap_err();
        validate_cpu_list("1-4294967295", &present).unwrap_err();
        validate_cpu_list("2,3", &[0, 2, 4].into()).unwrap_err();
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
