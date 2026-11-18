use std::collections::HashMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{BuiltInMeasure as _, systems::CpuUtilization},
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
        storage::{fio_json::AdapterStorageFioJson, fio_text::AdapterStorageFioText},
    },
    results::adapter_results::AdapterResults,
};

/// fio at version 1: its JSON, then its `normal` text.
pub struct AdapterStorageFio;

impl Adaptable for AdapterStorageFio {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterStorageFioJson::parse(input, settings)
            .or_else(|| AdapterStorageFioText::parse(input, settings))
    }
}

/// The results of a fio report's jobs, each named by its job name.
///
/// Jobs that repeat a name, as `--numjobs` without `--group_reporting` writes them, carry `@job`,
/// their place among the jobs of that name, so a failed or idle job keeps its place.
///
/// `unwritten` counts the failed jobs the report left out. Their names are unknown, so while
/// any is left out, the place of a job that repeats a name is unknown too, and it is skipped
/// and counted.
pub(super) fn results(jobs: &[Job], unwritten: usize, adapter: Adapter) -> Option<AdapterResults> {
    let mut repeats = HashMap::<&str, usize>::new();
    for job in jobs {
        *repeats.entry(&job.name).or_default() += 1;
    }
    let mut places = HashMap::<&str, usize>::new();
    let mut results = LeafResults::default();
    for _ in 0..unwritten {
        results.skip_benchmark();
    }
    for job in jobs {
        let place = places.entry(&job.name).or_default();
        let index = *place;
        *place += 1;
        let run = match &job.outcome {
            Outcome::Ran(run) => run,
            Outcome::Failed => {
                results.skip_benchmark();
                continue;
            },
            Outcome::Idle => continue,
        };
        let repeated = repeats
            .get(job.name.as_str())
            .is_some_and(|repeats| *repeats > 1);
        let Ok(benchmark) = job.name.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            continue;
        };
        if repeated && unwritten > 0 {
            results.skip_benchmark();
            continue;
        }
        let mut options = run.options.clone();
        if repeated {
            options.insert("@job".to_owned(), index.into());
        }
        let mut insert = |key: Option<(&str, &str)>, metrics: &Metrics| {
            let mut parameters = options.clone();
            if let Some((key, value)) = key {
                parameters.insert(key.to_owned(), value.into());
            }
            if let Some(parameters) = parameter_set(parameters) {
                metrics.write(&mut results.variant(benchmark.clone(), parameters));
            } else {
                results.skip_variant();
            }
        };
        insert(None, &run.metrics);
        for (direction, metrics) in &run.directions {
            insert(Some(("@direction", direction.as_str())), metrics);
        }
        if let Some(Cpu { user, system }) = run.cpu {
            for (cpu, value) in [("user", user), ("system", system)] {
                let mut metrics = Metrics::default();
                metrics.insert(&CpuUtilization::name_id(), "value", value);
                insert(Some(("@cpu", cpu)), &metrics);
            }
        }
    }
    results.finish_with_skips(adapter)
}

/// A job of a fio report.
pub(super) struct Job {
    pub name: String,
    pub outcome: Outcome,
}

/// How a job ended: it ran; it failed with no count of its errors, so it is skipped and counted;
/// or it did no I/O, so it is skipped uncounted.
pub(super) enum Outcome {
    Ran(Run),
    Failed,
    Idle,
}

/// What a job that ran reports.
pub(super) struct Run {
    /// The workload options the user set, each under its `@` key.
    pub options: serde_json::Map<String, serde_json::Value>,
    /// The quantities fio reports per job.
    pub metrics: Metrics,
    /// The quantities fio reports per direction, for each direction that did I/O.
    pub directions: Vec<(Direction, Metrics)>,
    pub cpu: Option<Cpu>,
}

/// A direction of I/O, as fio names it.
#[derive(Clone, Copy)]
pub(super) enum Direction {
    Read,
    Write,
    Trim,
    Sync,
    /// Every direction at once, under `unified_rw_reporting`.
    Mixed,
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Trim => "trim",
            Self::Sync => "sync",
            Self::Mixed => "mixed",
        }
    }
}

/// A job's user and system CPU utilization, in percent.
#[derive(Clone, Copy)]
pub(super) struct Cpu {
    pub user: f64,
    pub system: f64,
}

/// The metrics of one variant, each by its measure and its name, and how many the leaf could
/// not read.
#[derive(Default)]
pub(super) struct Metrics {
    values: Vec<(MeasureNameId, String, f64)>,
    skipped: usize,
}

impl Metrics {
    pub(super) fn insert(&mut self, measure: &MeasureNameId, metric: &str, value: f64) {
        self.values
            .push((measure.clone(), metric.to_owned(), value));
    }

    /// A latency's minimum, maximum, mean, and standard deviation.
    pub(super) fn insert_summary(
        &mut self,
        measure: &MeasureNameId,
        [min, max, mean, std_dev]: [f64; 4],
    ) {
        for (metric, value) in [
            ("min", min),
            ("max", max),
            ("mean", mean),
            ("std_dev", std_dev),
        ] {
            self.insert(measure, metric, value);
        }
    }

    /// The percentile at `percent`, but for 0 and 100, whose bin estimates yield to fio's exact
    /// minimum and maximum.
    pub(super) fn insert_percentile(&mut self, measure: &MeasureNameId, percent: &str, value: f64) {
        let Some(metric) = percentile_name(percent) else {
            self.skip_metric();
            return;
        };
        if metric != "min" && metric != "max" {
            self.values.push((measure.clone(), metric, value));
        }
    }

    pub(super) fn skip_metric(&mut self) {
        self.skipped += 1;
    }

    fn write(&self, variant: &mut Variant<'_>) {
        for (measure, metric, value) in &self.values {
            variant.insert(measure.clone(), metric, *value);
        }
        for _ in 0..self.skipped {
            variant.skip_metric();
        }
    }
}
