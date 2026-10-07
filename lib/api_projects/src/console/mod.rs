//! Endpoints shaped for the console, which change with it and so stay out of the published API.

use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use bencher_json::{
    AlertUuid, BenchmarkName, BenchmarkSlug, Boundary, MeasureSlug, MeasureUuid, ParameterSet,
    ResourceName, SampleSize, VariantUuid, Window,
};
use bencher_json::{
    BenchmarkUuid, DateTime, DateTimeMillis, GitHash, ModelTest, ModelUuid, ReportUuid,
    ThresholdUuid,
    project::{
        alert::AlertStatus,
        boundary::BoundaryLimit,
        console::{
            JsonConsoleAlert, JsonConsoleAlertPoint, JsonConsoleBenchmark, JsonConsoleMeasure,
            JsonConsoleModel, JsonConsolePointReport, JsonConsolePoints, JsonConsoleSeries,
            JsonConsoleVariant,
        },
        head::VersionNumber,
        report::Iteration,
    },
};
use bencher_schema::model::project::{
    benchmark::BenchmarkId, measure::MeasureId, report::ReportId, threshold::ThresholdId,
    variant::VariantId,
};

pub mod alerts;
pub mod report;

fn before(time: DateTime, duration: Duration) -> Option<DateTime> {
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    DateTime::try_from(time.timestamp().checked_sub(seconds)?).ok()
}

#[derive(Clone)]
struct PointReport {
    id: ReportId,
    uuid: ReportUuid,
    start_time: DateTime,
    end_time: DateTime,
    created: DateTime,
    version: VersionNumber,
    hash: Option<GitHash>,
}

/// Points run along x by report start time; the rest only breaks ties.
type ReportOrder = (i64, i64, i64, ReportId);

impl PointReport {
    fn order(&self) -> ReportOrder {
        (
            self.start_time.timestamp(),
            self.end_time.timestamp(),
            self.created.timestamp(),
            self.id,
        )
    }
}

/// The shared x of a response: one point per report and iteration that any line
/// has a value at.
#[derive(Default)]
struct PointsBuilder {
    reports: HashMap<ReportId, PointReport>,
    points: BTreeSet<(ReportOrder, u32)>,
}

/// The finished x, and where each report and iteration landed on it.
#[derive(Default)]
struct Points {
    json: JsonConsolePoints,
    reports: Vec<JsonConsolePointReport>,
    index: HashMap<(ReportId, Iteration), usize>,
}

impl PointsBuilder {
    fn add(&mut self, report: &PointReport, iteration: Iteration) {
        self.points.insert((report.order(), iteration.0));
        self.reports
            .entry(report.id)
            .or_insert_with(|| report.clone());
    }

    fn build(self) -> Points {
        let Self { reports, points } = self;
        let mut json = JsonConsolePoints::default();
        let mut iterations = Vec::with_capacity(points.len());
        let mut table = Vec::new();
        let mut table_index = HashMap::new();
        let mut index = HashMap::with_capacity(points.len());
        for (position, ((start_time, _, _, report_id), iteration)) in points.into_iter().enumerate()
        {
            let iteration = Iteration(iteration);
            let Some(report) = reports.get(&report_id) else {
                debug_assert!(false, "every point's report was added with it");
                continue;
            };
            let report_index = *table_index.entry(report_id).or_insert_with(|| {
                table.push(JsonConsolePointReport {
                    uuid: report.uuid,
                    version: report.version,
                    hash: report.hash.clone(),
                });
                to_index(table.len() - 1)
            });
            json.x.push(millis(start_time));
            json.report.push(report_index);
            iterations.push(iteration);
            index.insert((report_id, iteration), position);
        }
        if iterations.iter().any(|iteration| iteration.0 != 0) {
            json.iteration = Some(iterations);
        }
        Points {
            json,
            reports: table,
            index,
        }
    }
}

fn millis(timestamp: i64) -> DateTimeMillis {
    DateTime::try_from(timestamp).map_or_else(
        |_| {
            debug_assert!(false, "a stored time is always valid");
            DateTimeMillis::from(DateTime::default())
        },
        Into::into,
    )
}

fn to_index(position: usize) -> u32 {
    u32::try_from(position).unwrap_or(u32::MAX)
}

/// One line's columns, filled point by point.
struct SeriesBuilder {
    y: Vec<Option<f64>>,
    baseline: Vec<Option<f64>>,
    lower: Vec<Option<f64>>,
    upper: Vec<Option<f64>>,
    alerts: Vec<JsonConsoleAlertPoint>,
}

impl SeriesBuilder {
    fn new(points: usize) -> Self {
        Self {
            y: vec![None; points],
            baseline: vec![None; points],
            lower: vec![None; points],
            upper: vec![None; points],
            alerts: Vec::new(),
        }
    }

    fn value(&mut self, point: usize, value: f64) {
        if let Some(y) = self.y.get_mut(point) {
            *y = Some(value);
        }
    }

    fn limits(&mut self, point: usize, limits: Limits, alert: Option<JsonConsoleAlert>) {
        let Limits {
            baseline,
            lower_limit,
            upper_limit,
        } = limits;
        for (column, value) in [
            (&mut self.baseline, baseline),
            (&mut self.lower, lower_limit),
            (&mut self.upper, upper_limit),
        ] {
            if let Some(cell) = column.get_mut(point) {
                *cell = value;
            }
        }
        if let Some(JsonConsoleAlert {
            uuid,
            limit,
            status,
        }) = alert
        {
            self.alerts.push(JsonConsoleAlertPoint {
                index: to_index(point),
                uuid,
                limit,
                status,
            });
        }
    }

    fn build(self) -> JsonConsoleSeries {
        let Self {
            y,
            baseline,
            lower,
            upper,
            mut alerts,
        } = self;
        alerts.sort_by_key(|alert| alert.index);
        let present =
            |column: Vec<Option<f64>>| column.iter().any(Option::is_some).then_some(column);
        JsonConsoleSeries {
            y,
            baseline: present(baseline),
            lower: present(lower),
            upper: present(upper),
            alerts,
        }
    }
}

/// What a boundary computed for one value.
#[derive(Clone, Copy)]
struct Limits {
    baseline: Option<f64>,
    lower_limit: Option<f64>,
    upper_limit: Option<f64>,
}

/// One threshold's check of one value: the boundary it computed, the model it
/// used, and the alert it raised, if any.
#[derive(Clone)]
struct Check {
    threshold_id: ThresholdId,
    threshold_uuid: ThresholdUuid,
    model: ModelRow,
    limits: Limits,
    alert: Option<JsonConsoleAlert>,
}

impl Check {
    /// Of the thresholds that checked one value, the line follows the one that
    /// alerted, and otherwise the oldest.
    fn choose(checks: Vec<Self>) -> Option<Self> {
        checks
            .into_iter()
            .min_by_key(|check| (check.alert.is_none(), check.threshold_id))
    }

    fn model_json(&self) -> JsonConsoleModel {
        let (uuid, test, min_sample_size, max_sample_size, window, lower_boundary, upper_boundary) =
            self.model;
        JsonConsoleModel {
            uuid,
            threshold: self.threshold_uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }

    /// How far the value has moved toward the side the threshold guards: positive
    /// is worse. `None` when there is no baseline to compare with.
    fn score(&self, value: f64) -> Option<f64> {
        let (_, _, _, _, _, lower_boundary, upper_boundary) = self.model;
        let baseline = self.limits.baseline?;
        // Against the magnitude, so a negative baseline keeps the guarded side.
        let delta = (value - baseline) / baseline.abs();
        match (lower_boundary.is_some(), upper_boundary.is_some()) {
            (true, true) => Some(delta.abs()),
            (false, true) => Some(delta),
            (true, false) => Some(-delta),
            (false, false) => None,
        }
        .filter(|score| score.is_finite())
    }
}

type ModelRow = (
    ModelUuid,
    ModelTest,
    Option<SampleSize>,
    Option<SampleSize>,
    Option<Window>,
    Option<Boundary>,
    Option<Boundary>,
);

type AlertRow = (AlertUuid, BoundaryLimit, AlertStatus);

fn alert_json(alert: Option<AlertRow>) -> Option<JsonConsoleAlert> {
    alert.map(|(uuid, limit, status)| JsonConsoleAlert {
        uuid,
        limit,
        status,
    })
}

type BenchmarkRow = (BenchmarkId, BenchmarkUuid, BenchmarkName, BenchmarkSlug);
type VariantRow = (VariantId, VariantUuid, ParameterSet);
type MeasureRow = (
    MeasureId,
    MeasureUuid,
    ResourceName,
    MeasureSlug,
    ResourceName,
);

/// The dimensions a response refers to, each once, by position.
#[derive(Default)]
struct Tables {
    benchmarks: Vec<JsonConsoleBenchmark>,
    benchmark_index: HashMap<BenchmarkId, u32>,
    variants: Vec<JsonConsoleVariant>,
    variant_index: HashMap<VariantId, u32>,
    measures: Vec<JsonConsoleMeasure>,
    measure_index: HashMap<MeasureId, u32>,
    models: Vec<JsonConsoleModel>,
    model_index: HashMap<ModelUuid, u32>,
}

impl Tables {
    fn benchmark(&mut self, row: &BenchmarkRow) -> u32 {
        let (id, uuid, name, slug) = row;
        *self.benchmark_index.entry(*id).or_insert_with(|| {
            self.benchmarks.push(JsonConsoleBenchmark {
                uuid: *uuid,
                name: name.clone(),
                slug: slug.clone(),
            });
            to_index(self.benchmarks.len() - 1)
        })
    }

    fn variant(&mut self, row: &VariantRow, benchmark: u32) -> u32 {
        let (id, uuid, parameters) = row;
        *self.variant_index.entry(*id).or_insert_with(|| {
            self.variants.push(JsonConsoleVariant {
                uuid: *uuid,
                benchmark,
                parameters: parameters.clone(),
            });
            to_index(self.variants.len() - 1)
        })
    }

    fn measure(&mut self, row: &MeasureRow) -> u32 {
        let (id, uuid, name, slug, units) = row;
        *self.measure_index.entry(*id).or_insert_with(|| {
            self.measures.push(JsonConsoleMeasure {
                uuid: *uuid,
                name: name.clone(),
                slug: slug.clone(),
                units: units.clone(),
            });
            to_index(self.measures.len() - 1)
        })
    }

    fn model(&mut self, check: &Check) -> u32 {
        let json = check.model_json();
        *self.model_index.entry(json.uuid).or_insert_with(|| {
            self.models.push(json);
            to_index(self.models.len() - 1)
        })
    }
}
