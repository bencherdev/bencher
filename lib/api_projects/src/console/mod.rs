//! Endpoints shaped for the console, which change with it and so stay out of the published API.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    hash::Hash,
    time::Duration,
};

use bencher_json::{
    AlertUuid, BenchmarkName, BenchmarkSlug, Boundary, MeasureSlug, MeasureUuid, ParameterSet,
    ResourceName, SampleSize, VariantUuid, Window,
};
use bencher_json::{
    BenchmarkUuid, Clock, DateTime, DateTimeMillis, GitHash, ModelTest, ModelUuid, ReportUuid,
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
use bencher_schema::model::{
    project::{
        benchmark::BenchmarkId, measure::MeasureId, report::ReportId, threshold::ThresholdId,
        variant::VariantId,
    },
    user::actor::ApiActor,
};

pub mod alerts;
pub mod dimensions;
pub mod latest;
pub mod perf;
pub mod report;
pub mod thresholds;

/// The longest that three calendar months run, and so the furthest back an
/// unauthenticated plot reaches from now.
const PUBLIC_REACH: Duration = Duration::from_hours(92 * 24);

/// Move a start that reaches further back than an unauthenticated request may up
/// to the limit, and say whether it moved.
fn clamp_start(clock: &Clock, api_actor: &ApiActor, start_time: DateTime) -> (DateTime, bool) {
    if api_actor.is_auth() {
        return (start_time, false);
    }
    match before(clock.now(), PUBLIC_REACH) {
        Some(floor) if start_time.timestamp() < floor.timestamp() => (floor, true),
        Some(_) | None => (start_time, false),
    }
}

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
            index: None,
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
        model_json(self.threshold_uuid, self.model)
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

fn model_json(threshold: ThresholdUuid, model: ModelRow) -> JsonConsoleModel {
    let (uuid, test, min_sample_size, max_sample_size, window, lower_boundary, upper_boundary) =
        model;
    JsonConsoleModel {
        uuid,
        threshold,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
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
        self.model_json(check.model_json())
    }

    fn model_json(&mut self, json: JsonConsoleModel) -> u32 {
        *self.model_index.entry(json.uuid).or_insert_with(|| {
            self.models.push(json);
            to_index(self.models.len() - 1)
        })
    }
}

fn unique<T: Copy + Eq + Hash, I: Iterator<Item = T>>(items: I) -> Vec<T> {
    let mut seen = HashSet::new();
    items.filter(|item| seen.insert(*item)).collect()
}

/// A history cut down to what its row can draw, on the points some line keeps.
struct Thinned {
    points: JsonConsolePoints,
    reports: Vec<JsonConsolePointReport>,
    series: Vec<JsonConsoleSeries>,
}

/// Thin every series with more than `limit` values to the lowest and highest
/// value of each of `limit / 2` equal stretches of its values, every alerting
/// point, and the report's own points, then keep only the points some series
/// still has a value at.
fn thin(points: Points, series: Vec<JsonConsoleSeries>, report: ReportId, limit: usize) -> Thinned {
    let Points {
        json,
        reports,
        index,
    } = points;
    let own = index
        .iter()
        .filter(|((report_id, _), _)| *report_id == report)
        .map(|(_, point)| *point)
        .collect::<BTreeSet<_>>();
    let kept = series
        .iter()
        .map(|series| kept_points(series, &own, limit))
        .collect::<Vec<_>>();
    let used = series
        .iter()
        .zip(&kept)
        .flat_map(|(series, kept)| match kept {
            Some(kept) => kept.clone(),
            None => present(series),
        })
        .collect::<BTreeSet<_>>();
    if kept.iter().all(Option::is_none) && used.len() == json.x.len() {
        return Thinned {
            points: json,
            reports,
            series,
        };
    }
    let position = used
        .iter()
        .enumerate()
        .map(|(position, point)| (*point, position))
        .collect::<HashMap<_, _>>();
    let (points, reports) = keep_points(json, &reports, &used);
    let series = series
        .into_iter()
        .zip(kept)
        .map(|(series, kept)| match kept {
            Some(kept) => {
                let index = kept
                    .iter()
                    .filter_map(|point| position.get(point).copied().map(to_index))
                    .collect();
                keep_series(series, &kept, Some(index))
            },
            None => keep_series(series, &used.iter().copied().collect::<Vec<_>>(), None),
        })
        .collect();
    Thinned {
        points,
        reports,
        series,
    }
}

/// The points a series has a value at.
fn present(series: &JsonConsoleSeries) -> Vec<usize> {
    series
        .y
        .iter()
        .enumerate()
        .filter_map(|(point, y)| y.is_some().then_some(point))
        .collect()
}

/// The points a thinned series keeps, or `None` when it fits.
fn kept_points(
    series: &JsonConsoleSeries,
    own: &BTreeSet<usize>,
    limit: usize,
) -> Option<Vec<usize>> {
    let values = present(series);
    if values.len() <= limit {
        return None;
    }
    let stretches = limit.div_euclid(2).max(1);
    let mut kept = BTreeSet::new();
    for stretch in 0..stretches {
        let bound = |stretch: usize| {
            stretch
                .saturating_mul(values.len())
                .checked_div(stretches)
                .unwrap_or_default()
        };
        let Some(points) = values.get(bound(stretch)..bound(stretch + 1)) else {
            continue;
        };
        let value = |point: &usize| series.y.get(*point).copied().flatten();
        let mut lowest: Option<(usize, f64)> = None;
        let mut highest: Option<(usize, f64)> = None;
        for point in points {
            let Some(value) = value(point) else {
                continue;
            };
            if lowest.is_none_or(|(_, low)| value < low) {
                lowest = Some((*point, value));
            }
            if highest.is_none_or(|(_, high)| value > high) {
                highest = Some((*point, value));
            }
        }
        kept.extend(lowest.into_iter().chain(highest).map(|(point, _)| point));
    }
    kept.extend(series.alerts.iter().map(|alert| alert.index as usize));
    kept.extend(
        own.iter()
            .copied()
            .filter(|point| values.binary_search(point).is_ok()),
    );
    Some(kept.into_iter().collect())
}

/// The shared x and its reports table on only the points in `used`.
fn keep_points(
    json: JsonConsolePoints,
    reports: &[JsonConsolePointReport],
    used: &BTreeSet<usize>,
) -> (JsonConsolePoints, Vec<JsonConsolePointReport>) {
    let JsonConsolePoints {
        x,
        report,
        iteration,
    } = json;
    let mut table = Vec::new();
    let mut table_index = HashMap::new();
    let mut points = JsonConsolePoints::default();
    let mut iterations = Vec::new();
    for point in used {
        let (Some(x), Some(report_index)) = (x.get(*point), report.get(*point)) else {
            continue;
        };
        let Some(entry) = reports.get(*report_index as usize) else {
            continue;
        };
        let new_index = *table_index.entry(*report_index).or_insert_with(|| {
            table.push(entry.clone());
            to_index(table.len() - 1)
        });
        points.x.push(*x);
        points.report.push(new_index);
        if let Some(iteration) = iteration
            .as_ref()
            .and_then(|iteration| iteration.get(*point))
        {
            iterations.push(*iteration);
        }
    }
    if iterations.iter().any(|iteration| iteration.0 != 0) {
        points.iteration = Some(iterations);
    }
    (points, table)
}

/// A series on only the given points, with its alerts pointing into the columns
/// that remain.
fn keep_series(
    series: JsonConsoleSeries,
    points: &[usize],
    index: Option<Vec<u32>>,
) -> JsonConsoleSeries {
    let JsonConsoleSeries {
        index: _,
        y,
        baseline,
        lower,
        upper,
        alerts,
    } = series;
    let keep = |column: Vec<Option<f64>>| {
        points
            .iter()
            .map(|point| column.get(*point).copied().flatten())
            .collect::<Vec<_>>()
    };
    let alerts = alerts
        .into_iter()
        .filter_map(|alert| {
            let position = points.binary_search(&(alert.index as usize)).ok()?;
            Some(JsonConsoleAlertPoint {
                index: to_index(position),
                ..alert
            })
        })
        .collect();
    JsonConsoleSeries {
        index,
        y: keep(y),
        baseline: baseline.map(keep),
        lower: lower.map(keep),
        upper: upper.map(keep),
        alerts,
    }
}
