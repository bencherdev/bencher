use bencher_runner::{PerfEventParanoid, Swappiness, TuningConfig};
use clap::Args;

/// Host tuning flags shared by `run` and `up` subcommands.
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI flags map to independent tuning knobs"
)]
#[derive(Args, Debug)]
pub struct CliTuning {
    /// Disable all host tuning optimizations.
    #[arg(long)]
    pub no_tuning: bool,

    /// Keep ASLR enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub aslr: bool,

    /// Keep NMI watchdog enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub nmi_watchdog: bool,

    /// Keep SMT / hyper-threading enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub smt: bool,

    /// Keep turboboost enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub turbo: bool,

    /// Keep automatic NUMA balancing enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub numa_balancing: bool,

    /// Keep timer migration enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub timer_migration: bool,

    /// Keep the soft lockup watchdog enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub soft_watchdog: bool,

    /// Keep kernel samepage merging (KSM) enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub ksm: bool,

    /// Keep deep C-states enabled (default: disabled for benchmarks).
    #[arg(long)]
    pub cstates: bool,

    /// Do not steer device IRQs and kernel workqueues to housekeeping cores.
    #[arg(long)]
    pub no_irq_steering: bool,

    /// Set swappiness value (default: 10).
    #[arg(long)]
    pub swappiness: Option<u32>,

    /// Set CPU scaling governor (default: performance).
    #[arg(long)]
    pub governor: Option<String>,

    /// Set `perf_event_paranoid` value (default: -1).
    #[arg(long, allow_hyphen_values = true)]
    pub perf_event_paranoid: Option<i32>,
}

impl TryFrom<CliTuning> for TuningConfig {
    type Error = bencher_runner::RunnerError;

    fn try_from(cli: CliTuning) -> Result<Self, Self::Error> {
        if cli.no_tuning {
            return Ok(Self::disabled());
        }
        Ok(Self {
            disable_aslr: !cli.aslr,
            disable_nmi_watchdog: !cli.nmi_watchdog,
            swappiness: cli
                .swappiness
                .map(Swappiness::try_from)
                .transpose()?
                .or(Some(Swappiness::DEFAULT)),
            perf_event_paranoid: cli
                .perf_event_paranoid
                .map(PerfEventParanoid::try_from)
                .transpose()?
                .or(Some(PerfEventParanoid::DEFAULT)),
            governor: cli.governor.or_else(|| Some("performance".to_owned())),
            disable_smt: !cli.smt,
            disable_turbo: !cli.turbo,
            disable_numa_balancing: !cli.numa_balancing,
            disable_timer_migration: !cli.timer_migration,
            disable_soft_watchdog: !cli.soft_watchdog,
            disable_ksm: !cli.ksm,
            disable_cstates: !cli.cstates,
            steer_kernel_work: !cli.no_irq_steering,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cli() -> CliTuning {
        CliTuning {
            no_tuning: false,
            aslr: false,
            nmi_watchdog: false,
            smt: false,
            turbo: false,
            numa_balancing: false,
            timer_migration: false,
            soft_watchdog: false,
            ksm: false,
            cstates: false,
            no_irq_steering: false,
            swappiness: None,
            governor: None,
            perf_event_paranoid: None,
        }
    }

    #[test]
    fn default_flags_enable_all_tuning() {
        let config = TuningConfig::try_from(default_cli()).unwrap();
        assert!(config.disable_aslr);
        assert!(config.disable_nmi_watchdog);
        assert!(config.disable_smt);
        assert!(config.disable_turbo);
        assert!(config.disable_numa_balancing);
        assert!(config.disable_timer_migration);
        assert!(config.disable_soft_watchdog);
        assert!(config.disable_ksm);
        assert!(config.disable_cstates);
        assert!(config.steer_kernel_work);
        assert_eq!(config.swappiness, Some(Swappiness::DEFAULT));
        assert_eq!(config.perf_event_paranoid, Some(PerfEventParanoid::DEFAULT));
        assert_eq!(config.governor.as_deref(), Some("performance"));
    }

    #[test]
    fn no_tuning_disables_everything() {
        let cli = CliTuning {
            no_tuning: true,
            ..default_cli()
        };
        let config = TuningConfig::try_from(cli).unwrap();
        assert!(!config.disable_aslr);
        assert!(!config.disable_numa_balancing);
        assert!(!config.disable_cstates);
        assert!(!config.steer_kernel_work);
        assert_eq!(config.swappiness, None);
        assert_eq!(config.governor, None);
    }

    #[test]
    fn individual_keep_flags_disable_single_knobs() {
        let cli = CliTuning {
            numa_balancing: true,
            cstates: true,
            no_irq_steering: true,
            ..default_cli()
        };
        let config = TuningConfig::try_from(cli).unwrap();
        assert!(!config.disable_numa_balancing);
        assert!(!config.disable_cstates);
        assert!(!config.steer_kernel_work);
        // Everything else stays enabled
        assert!(config.disable_aslr);
        assert!(config.disable_timer_migration);
        assert!(config.disable_soft_watchdog);
        assert!(config.disable_ksm);
    }
}
