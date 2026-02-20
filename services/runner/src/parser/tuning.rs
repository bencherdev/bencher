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
        })
    }
}
