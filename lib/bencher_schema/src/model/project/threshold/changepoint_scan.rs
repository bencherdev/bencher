//! The changepoint detector.
//!
//! A scan reads a line as one point per version, runs Otava's full analysis once for each side its
//! changepoint model guards, and reconciles what it finds with the changepoints already on the line.

use std::collections::{BTreeMap, HashMap};

use bencher_changepoint::{Changepoint, ChangepointError, Parameters};
use bencher_json::{
    DateTime, MetricName, ModelTest, ModelUuid, ParameterFilter, ParameterSet,
    project::head::VersionNumber,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, OptionalExtension as _,
    QueryDsl as _, QueryResult, RunQueryDsl, SelectableHelper as _,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    sql_types::{Double, Integer, Text},
    sqlite::Sqlite,
};
use slog::Logger;

use super::{
    ThresholdId,
    changepoint::{ChangepointLine, ChangepointState, InsertChangepoint, QueryChangepoint},
    model::{ModelId, QueryModel},
};
use crate::{
    context::DbConnection,
    model::project::{
        branch::{BranchId, head::HeadId, version::VersionId},
        measure::MeasureId,
        testbed::TestbedId,
    },
    schema,
};

/// A line that a threshold's changepoint model watches, as a report hands it to the detector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchedLine {
    pub line: ChangepointLine,
    pub threshold_id: ThresholdId,
}

/// One point of a line: a version on its head, valued at the median of the line's metrics on
/// that version.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinePoint {
    pub version_id: VersionId,
    pub value: f64,
}

/// Read the points of a line, in version order: one for each version native to the head (above
/// its start point) that has a metric on the line from a report under the line's spec.
pub fn line_points(conn: &mut DbConnection, line: &ChangepointLine) -> QueryResult<Vec<LinePoint>> {
    let start_point = start_point_of(conn, line.head_id)?;
    // A head with a start point, as a pull request's is, holds few rows of its own, so its read
    // starts from the head. A head with no start point holds the line's whole history, so its read
    // starts from the variant's rows.
    let order = if start_point.is_some() {
        LineOrder::HeadFirst
    } else {
        LineOrder::VariantFirst
    };
    line_points_in_order(conn, line, start_point, order)
}

/// The number of the head's start point, if it has one.
fn start_point_of(conn: &mut DbConnection, head_id: HeadId) -> QueryResult<Option<VersionNumber>> {
    schema::head::table
        .inner_join(
            schema::head_version::table.on(schema::head_version::id
                .nullable()
                .eq(schema::head::start_point_id)),
        )
        .inner_join(
            schema::version::table.on(schema::version::id.eq(schema::head_version::version_id)),
        )
        .filter(schema::head::id.eq(head_id))
        .select(schema::version::number)
        .first::<VersionNumber>(conn)
        .optional()
}

fn line_points_in_order(
    conn: &mut DbConnection,
    line: &ChangepointLine,
    start_point: Option<VersionNumber>,
    order: LineOrder,
) -> QueryResult<Vec<LinePoint>> {
    let metrics = LineQuery {
        line,
        start_point,
        order,
    }
    .load::<(VersionId, f64)>(conn)?;

    let mut points = Vec::new();
    let mut values = Vec::new();
    let mut metrics = metrics.into_iter().peekable();
    while let Some((version_id, value)) = metrics.next() {
        values.push(value);
        if metrics
            .peek()
            .is_none_or(|(next_version_id, _)| *next_version_id != version_id)
        {
            if let Some(value) = median(&mut values) {
                points.push(LinePoint { version_id, value });
            }
            values.clear();
        }
    }
    Ok(points)
}

/// The median of the values: the middle one, or the mean of the middle two, and none for none.
fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let upper = values.len().checked_div(2)?;
    let high = *values.get(upper)?;
    if values.len().is_multiple_of(2) {
        let low = *values.get(upper.checked_sub(1)?)?;
        Some(f64::midpoint(low, high))
    } else {
        Some(high)
    }
}

/// Every metric of a line on a version native to its head, by version number and then version.
struct LineQuery<'a> {
    line: &'a ChangepointLine,
    /// The number of the head's start point, at or below which every version is a clone.
    start_point: Option<VersionNumber>,
    order: LineOrder,
}

/// The table a line's read starts from. Both orders read the same rows.
#[derive(Debug, Clone, Copy)]
enum LineOrder {
    /// The head's versions, then their reports on the testbed, then the variant's rows in them.
    HeadFirst,
    /// The variant's rows in the project, then their reports, then the versions the head holds.
    VariantFirst,
}

impl Query for LineQuery<'_> {
    type SqlType = (Integer, Double);
}

impl QueryId for LineQuery<'_> {
    type QueryId = ();
    // The optional filters and the order change the SQL, so the prepared statement cache has to
    // key on the text it builds rather than on this type.
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<Conn> RunQueryDsl<Conn> for LineQuery<'_> {}

impl QueryFragment<Sqlite> for LineQuery<'_> {
    fn walk_ast<'b>(&'b self, pass: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        match self.order {
            LineOrder::HeadFirst => self.head_first(pass),
            LineOrder::VariantFirst => self.variant_first(pass),
        }
    }
}

impl LineQuery<'_> {
    // The same join order as the metric detector's history, with the version for its number.
    fn head_first<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        let Self {
            line,
            start_point,
            order: _,
        } = self;
        let ChangepointLine {
            project_id: _,
            head_id,
            testbed_id,
            spec_id,
            benchmark_id,
            variant_id,
            measure_id,
            metric,
        } = line;
        pass.push_sql(
            "SELECT version.id, metric.value FROM head_version \
             CROSS JOIN version \
             CROSS JOIN report \
             CROSS JOIN report_benchmark INDEXED BY index_report_benchmark_variant_report \
             CROSS JOIN metric \
             WHERE head_version.head_id = ",
        );
        pass.push_bind_param::<Integer, _>(head_id)?;
        if let Some(start_point) = start_point {
            pass.push_sql(" AND version.number > ");
            pass.push_bind_param::<Integer, _>(start_point)?;
        }
        pass.push_sql(" AND report.testbed_id = ");
        pass.push_bind_param::<Integer, _>(testbed_id)?;
        if let Some(spec_id) = spec_id {
            pass.push_sql(" AND report.spec_id = ");
            pass.push_bind_param::<Integer, _>(spec_id)?;
        } else {
            pass.push_sql(" AND report.spec_id IS NULL");
        }
        pass.push_sql(" AND report_benchmark.benchmark_id = ");
        pass.push_bind_param::<Integer, _>(benchmark_id)?;
        pass.push_sql(" AND report_benchmark.variant_id = ");
        pass.push_bind_param::<Integer, _>(variant_id)?;
        pass.push_sql(" AND metric.measure_id = ");
        pass.push_bind_param::<Integer, _>(measure_id)?;
        pass.push_sql(" AND metric.name = ");
        pass.push_bind_param::<Text, _>(metric)?;
        pass.push_sql(
            " AND version.id = head_version.version_id \
             AND report.version_id = head_version.version_id \
             AND report_benchmark.report_id = report.id \
             AND metric.report_benchmark_id = report_benchmark.id \
             ORDER BY version.number, version.id",
        );
        Ok(())
    }

    // The variant's rows in the project lead, then the head's versions filter them. Left to itself,
    // the planner starts from every metric of the measure, so the cross joins hold this order.
    fn variant_first<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        let Self {
            line,
            start_point,
            order: _,
        } = self;
        let ChangepointLine {
            project_id: _,
            head_id,
            testbed_id,
            spec_id,
            benchmark_id,
            variant_id,
            measure_id,
            metric,
        } = line;
        pass.push_sql(
            "SELECT version.id, metric.value \
             FROM report_benchmark INDEXED BY index_report_benchmark_variant_report \
             CROSS JOIN report \
             CROSS JOIN version \
             CROSS JOIN head_version \
             CROSS JOIN metric \
             WHERE report_benchmark.variant_id = ",
        );
        pass.push_bind_param::<Integer, _>(variant_id)?;
        pass.push_sql(" AND report_benchmark.benchmark_id = ");
        pass.push_bind_param::<Integer, _>(benchmark_id)?;
        pass.push_sql(" AND report.testbed_id = ");
        pass.push_bind_param::<Integer, _>(testbed_id)?;
        if let Some(spec_id) = spec_id {
            pass.push_sql(" AND report.spec_id = ");
            pass.push_bind_param::<Integer, _>(spec_id)?;
        } else {
            pass.push_sql(" AND report.spec_id IS NULL");
        }
        if let Some(start_point) = start_point {
            pass.push_sql(" AND version.number > ");
            pass.push_bind_param::<Integer, _>(start_point)?;
        }
        pass.push_sql(" AND head_version.head_id = ");
        pass.push_bind_param::<Integer, _>(head_id)?;
        pass.push_sql(" AND metric.measure_id = ");
        pass.push_bind_param::<Integer, _>(measure_id)?;
        pass.push_sql(" AND metric.name = ");
        pass.push_bind_param::<Text, _>(metric)?;
        pass.push_sql(
            " AND report.id = report_benchmark.report_id \
             AND version.id = report.version_id \
             AND head_version.version_id = version.id \
             AND metric.report_benchmark_id = report_benchmark.id \
             ORDER BY version.number, version.id",
        );
        Ok(())
    }
}

/// What one scan of a line read and found, for [`Scan::write`] to reconcile.
#[derive(Debug)]
pub struct Scan {
    watched: WatchedLine,
    model_id: ModelId,
    /// The model's UUID, which, unlike its id, no later model is given.
    model_uuid: ModelUuid,
    /// The consecutive scans that confirm a candidate, the model's `confirmations`.
    confirmations: u32,
    on_line: Vec<QueryChangepoint>,
    found: Vec<Found>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("Failed to read the line: {0}")]
    Read(diesel::result::Error),
    #[error("Failed to analyse the line: {0}")]
    Analyse(ChangepointError),
}

impl Scan {
    /// Read the line and analyse it with its threshold's changepoint model as it stands now.
    /// There is no scan of a line whose head was replaced, whose threshold holds no changepoint
    /// model, or that has fewer points than the model's minimum, nor once `stopped` returns true
    /// before a pass.
    pub fn read(
        log: &Logger,
        conn: &mut DbConnection,
        watched: &WatchedLine,
        stopped: &dyn Fn() -> bool,
    ) -> Result<Option<Self>, ScanError> {
        if is_replaced(conn, watched.line.head_id).map_err(ScanError::Read)? {
            return Ok(None);
        }
        let Some(model) =
            LineModel::of_threshold(conn, watched.threshold_id).map_err(ScanError::Read)?
        else {
            return Ok(None);
        };
        let points = line_points(conn, &watched.line).map_err(ScanError::Read)?;
        if model
            .min_points
            .is_some_and(|min_points| points.len() < min_points)
        {
            return Ok(None);
        }
        let series = points.iter().map(|point| point.value).collect::<Vec<_>>();
        let mut found = Vec::new();
        for (side, boundary) in [(Side::Upper, model.upper), (Side::Lower, model.lower)] {
            let Some(min_magnitude) = boundary else {
                continue;
            };
            if stopped() {
                return Ok(None);
            }
            let parameters = Parameters {
                window_len: model.window,
                max_pvalue: model.significance,
                min_magnitude,
            };
            let changepoints =
                bencher_changepoint::detect(&series, parameters).map_err(ScanError::Analyse)?;
            found.extend(kept_on_side(
                log,
                &watched.line,
                &points,
                side,
                changepoints,
            ));
        }
        let on_line = QueryChangepoint::on_line(conn, &watched.line)
            .map_err(ScanError::Read)?
            .into_iter()
            .filter(|changepoint| changepoint.threshold_id == watched.threshold_id)
            .collect();
        Ok(Some(Self {
            watched: watched.clone(),
            model_id: model.id,
            model_uuid: model.uuid,
            confirmations: model.confirmations,
            on_line,
            found,
        }))
    }

    /// Reconcile what the scan found with the line's changepoints, inside the caller's write
    /// transaction. A head replaced since the read, or a threshold that no longer holds the model
    /// the scan read, leaves the line as it is.
    /// Returns whether the line was reconciled.
    pub fn write(self, conn: &mut DbConnection, now: DateTime) -> QueryResult<bool> {
        let Self {
            watched,
            model_id,
            model_uuid,
            confirmations,
            on_line,
            mut found,
        } = self;
        if is_replaced(conn, watched.line.head_id)?
            || !holds_model(conn, watched.threshold_id, model_uuid)?
        {
            return Ok(false);
        }
        for changepoint in on_line {
            // A withdrawn changepoint is history, so a step that returns to its version is a new
            // candidate.
            if changepoint.status == ChangepointState::Withdrawn {
                continue;
            }
            // Confirmation starts again under the threshold's current model.
            if changepoint.status == ChangepointState::Candidate && changepoint.model_id != model_id
            {
                QueryChangepoint::delete_candidate(conn, changepoint.id)?;
                continue;
            }
            let Some(side) = Side::of(changepoint.mean_before, changepoint.mean_after) else {
                continue;
            };
            let found_again = found
                .iter()
                .position(|found| found.side == side && found.version_id == changepoint.version_id)
                .map(|index| found.remove(index));
            match (changepoint.status, found_again) {
                (ChangepointState::Candidate, Some(again)) => {
                    let scans = u32::try_from(QueryChangepoint::scans(conn, changepoint.id)?.len())
                        .unwrap_or(u32::MAX)
                        .saturating_add(1);
                    if scans >= confirmations {
                        QueryChangepoint::confirm(conn, changepoint.id, &again.changepoint, now)?;
                    } else {
                        QueryChangepoint::count_scan(
                            conn,
                            changepoint.id,
                            &again.changepoint,
                            now,
                        )?;
                    }
                },
                (ChangepointState::Candidate, None) => {
                    QueryChangepoint::delete_candidate(conn, changepoint.id)?;
                },
                (ChangepointState::Active, None) => {
                    QueryChangepoint::withdraw(conn, changepoint.id, now)?;
                },
                (ChangepointState::Active, Some(_))
                | (
                    ChangepointState::Dismissed
                    | ChangepointState::Silenced
                    | ChangepointState::Withdrawn,
                    _,
                ) => {},
            }
        }
        for Found {
            version_id,
            changepoint,
            ..
        } in found
        {
            let insert = if confirmations > 1 {
                InsertChangepoint::candidate
            } else {
                InsertChangepoint::confirmed
            };
            insert(
                conn,
                watched.line.clone(),
                version_id,
                watched.threshold_id,
                model_id,
                &changepoint,
                now,
            )?;
        }
        Ok(true)
    }
}

/// Whether the head was replaced, so its line is settled.
fn is_replaced(conn: &mut DbConnection, head_id: HeadId) -> QueryResult<bool> {
    schema::head::table
        .filter(schema::head::id.eq(head_id))
        .select(schema::head::replaced)
        .first::<Option<DateTime>>(conn)
        .map(|replaced| replaced.is_some())
}

/// Whether the threshold still holds the changepoint model in its slot. A deleted threshold holds
/// none, and a threshold made again with its ids holds another model.
fn holds_model(
    conn: &mut DbConnection,
    threshold_id: ThresholdId,
    model_uuid: ModelUuid,
) -> QueryResult<bool> {
    schema::threshold::table
        .inner_join(
            schema::model::table.on(schema::model::id
                .nullable()
                .eq(schema::threshold::changepoint_model_id)),
        )
        .filter(schema::threshold::id.eq(threshold_id))
        .select(schema::model::uuid)
        .first::<ModelUuid>(conn)
        .optional()
        .map(|held| held == Some(model_uuid))
}

/// The changepoints of one pass that land on a point of the line, can be stored, and are on the
/// pass's side. A changepoint is checked before its side is read, so every one that cannot be
/// stored is logged, a NaN mean among them.
fn kept_on_side(
    log: &Logger,
    line: &ChangepointLine,
    points: &[LinePoint],
    side: Side,
    changepoints: Vec<Changepoint>,
) -> Vec<Found> {
    changepoints
        .into_iter()
        .filter_map(|changepoint| {
            let point = points.get(changepoint.index)?;
            (is_storable(log, line, point.version_id, &changepoint)
                && Side::of(changepoint.mean_before, changepoint.mean_after) == Some(side))
            .then_some(Found {
                side,
                version_id: point.version_id,
                changepoint,
            })
        })
        .collect()
}

/// Whether the changepoint can be stored and served. Its t statistic may be infinite, as it is
/// between 2 constant sides, and is stored as none. A refused one is logged.
fn is_storable(
    log: &Logger,
    line: &ChangepointLine,
    version_id: VersionId,
    changepoint: &Changepoint,
) -> bool {
    let Changepoint {
        index: _,
        mean_before,
        mean_after,
        stddev_before,
        stddev_after,
        pvalue,
        tstatistic,
        degrees_of_freedom: _,
    } = *changepoint;
    let storable = [mean_before, mean_after, stddev_before, stddev_after, pvalue]
        .into_iter()
        .all(f64::is_finite)
        && !tstatistic.is_nan();
    if !storable {
        slog::warn!(log, "Refused a changepoint with a statistic that cannot be stored";
            "line" => ?line,
            "version_id" => ?version_id,
            "mean_before" => mean_before,
            "mean_after" => mean_after,
            "stddev_before" => stddev_before,
            "stddev_after" => stddev_after,
            "pvalue" => pvalue,
            "tstatistic" => tstatistic,
        );
    }
    storable
}

/// A threshold's changepoint model, as a scan reads it.
#[derive(Debug)]
struct LineModel {
    id: ModelId,
    uuid: ModelUuid,
    /// Otava's window length, the model's `max_sample_size`.
    window: usize,
    /// Otava's `max_pvalue` for each pass.
    significance: f64,
    /// The fewest points a line is analysed with, the model's `min_sample_size`.
    min_points: Option<usize>,
    /// The consecutive scans that confirm a candidate, the model's `confirmations`.
    confirmations: u32,
    upper: Option<f64>,
    lower: Option<f64>,
}

impl LineModel {
    /// The changepoint model the threshold holds now, if it holds a complete one.
    fn of_threshold(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
    ) -> QueryResult<Option<Self>> {
        let model = schema::threshold::table
            .inner_join(
                schema::model::table.on(schema::model::id
                    .nullable()
                    .eq(schema::threshold::changepoint_model_id)),
            )
            .filter(schema::threshold::id.eq(threshold_id))
            .select(QueryModel::as_select())
            .first::<QueryModel>(conn)
            .optional()?;
        let Some(QueryModel {
            id,
            uuid,
            test: ModelTest::Changepoint,
            min_sample_size,
            max_sample_size: Some(max_sample_size),
            lower_boundary,
            upper_boundary,
            significance: Some(significance),
            confirmations: Some(confirmations),
            ..
        }) = model
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            id,
            uuid,
            window: usize::from(max_sample_size),
            significance: f64::from(significance),
            min_points: min_sample_size.map(usize::from),
            confirmations: u32::from(confirmations),
            upper: upper_boundary.map(f64::from),
            lower: lower_boundary.map(f64::from),
        }))
    }
}

/// A changepoint one pass found, on the side whose boundary it passed.
#[derive(Debug)]
struct Found {
    side: Side,
    version_id: VersionId,
    changepoint: Changepoint,
}

/// A guarded side of a changepoint model: an upper boundary watches increases, a lower one
/// decreases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Upper,
    Lower,
}

impl Side {
    /// The side a step from `mean_before` to `mean_after` is on, or none for no step.
    fn of(mean_before: f64, mean_after: f64) -> Option<Self> {
        if mean_after > mean_before {
            Some(Self::Upper)
        } else if mean_after < mean_before {
            Some(Self::Lower)
        } else {
            None
        }
    }
}

/// The thresholds of a branch and testbed whose changepoint slot holds a model, for a report to
/// find the lines they watch.
#[derive(Debug, Clone)]
pub struct ChangepointWatch {
    pub threshold_id: ThresholdId,
    /// The variants this threshold watches. `None` watches every variant.
    pub parameters: Option<ParameterFilter>,
    /// The name this threshold watches, `value` when it names none.
    pub metric: MetricName,
}

impl ChangepointWatch {
    pub fn load(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> QueryResult<HashMap<MeasureId, Vec<Self>>> {
        let thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::changepoint_model_id.is_not_null())
            .select((
                schema::threshold::id,
                schema::threshold::measure_id,
                schema::threshold::parameters,
                schema::threshold::metric,
            ))
            .load::<(
                ThresholdId,
                MeasureId,
                Option<ParameterFilter>,
                Option<MetricName>,
            )>(conn)?;
        let mut by_measure: HashMap<MeasureId, Vec<Self>> = HashMap::new();
        for (threshold_id, measure_id, parameters, metric) in thresholds {
            by_measure.entry(measure_id).or_default().push(Self {
                threshold_id,
                parameters,
                metric: metric.unwrap_or_else(MetricName::value),
            });
        }
        Ok(by_measure)
    }

    pub fn watches(&self, variant: &ParameterSet) -> bool {
        self.parameters
            .as_ref()
            .is_none_or(|parameters| parameters.matches(variant))
    }

    /// The watches of a measure that watch a variant with these parameters, on a metric the
    /// report named.
    pub fn watching<'a, V>(
        watches: &'a [Self],
        variant: &'a ParameterSet,
        named: &'a BTreeMap<MetricName, V>,
    ) -> impl Iterator<Item = &'a Self> {
        watches
            .iter()
            .filter(move |watch| watch.watches(variant) && named.contains_key(&watch.metric))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        cell::Cell,
        fmt::{self, Write as _},
        sync::{Arc, Mutex},
    };

    use bencher_changepoint::Changepoint;
    use bencher_json::{DateTime, MetricName, ModelSlot, ModelTest, ParameterFilter, ParameterSet};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
    use pretty_assertions::assert_eq;
    use slog::{Drain, KV as _, Logger, OwnedKVList, Record};

    use super::{
        ChangepointWatch, LineOrder, LinePoint, Scan, Side, WatchedLine, is_storable, kept_on_side,
        line_points, line_points_in_order, start_point_of,
    };
    use crate::{
        context::DbConnection,
        macros::sql::last_insert_rowid,
        model::{
            project::{
                branch::{head::HeadId, version::VersionId},
                report::ReportId,
                threshold::{
                    QueryThreshold,
                    changepoint::{ChangepointLine, ChangepointState, QueryChangepoint},
                    model::{ModelId, QueryModel},
                },
            },
            spec::SpecId,
        },
        schema,
        test_util::{
            BranchIds, ChangepointFixture, CreateSpecArgs, MEASURED_STEP, create_base_entities,
            create_benchmark, create_branch_with_head, create_changepoint_fixture,
            create_head_version, create_measure, create_metric, create_named_metric, create_report,
            create_report_benchmark_for_variant, create_spec, create_testbed,
            create_threshold_with_dimensions, create_variant, create_version, get_empty_variant,
            set_report_spec, setup_test_db,
        },
    };

    const NOW: DateTime = DateTime::TEST;
    /// A small, fixed noise on a level of 100, so no two sides of a step are constant.
    const NOISE: [f64; 8] = [0.4, -0.3, 0.1, -0.5, 0.3, -0.1, 0.5, -0.4];

    /// One line of one threshold's changepoint model, on the head of `main`, with a report per
    /// version that the test adds.
    pub(crate) struct Line {
        conn: DbConnection,
        fixture: ChangepointFixture,
        branch: BranchIds,
        /// Each version of the head by its number.
        versions: Vec<VersionId>,
        /// How many versions hold a report so far.
        next: usize,
        uuids: u32,
        log: Logger,
    }

    impl Line {
        fn new() -> Self {
            Self::with_log(Logger::root(slog::Discard, slog::o!()))
        }

        fn with_log(log: Logger) -> Self {
            Self::on(setup_test_db(), log)
        }

        /// A line on a migrated database.
        pub(crate) fn on(mut conn: DbConnection, log: Logger) -> Self {
            let base = create_base_entities(&mut conn);
            let branch = create_branch_with_head(
                &mut conn,
                base.project_id,
                "00000000-0000-0000-0000-000000000010",
                "main",
                "main",
                "00000000-0000-0000-0000-000000000011",
            );
            // The fixture's own version is number 0, the first of the line.
            let fixture = create_changepoint_fixture(&mut conn, base.project_id, branch, 0);
            Self {
                conn,
                versions: vec![fixture.version_id],
                fixture,
                branch,
                next: 0,
                uuids: 0,
                log,
            }
        }

        fn uuid(&mut self) -> String {
            self.uuids += 1;
            format!("00000000-0000-0000-0d00-{:012}", self.uuids)
        }

        pub(crate) fn watched(&self) -> WatchedLine {
            WatchedLine {
                line: self.fixture.line.clone(),
                threshold_id: self.fixture.threshold_id,
            }
        }

        /// The version of the head at `number`, made with every number below it.
        fn version(&mut self, number: usize) -> VersionId {
            while self.versions.len() <= number {
                let uuid = self.uuid();
                let version_number = i32::try_from(self.versions.len()).expect("a number");
                let version_id = create_version(
                    &mut self.conn,
                    self.fixture.line.project_id,
                    &uuid,
                    version_number,
                    None,
                );
                create_head_version(&mut self.conn, self.branch.head_id, version_id);
                self.versions.push(version_id);
            }
            self.versions[number]
        }

        /// A report at the version with one iteration for each value, on the line.
        fn report(&mut self, version_id: VersionId, values: &[f64]) -> ReportId {
            let uuid = self.uuid();
            let line = self.fixture.line.clone();
            let report_id = create_report(
                &mut self.conn,
                &uuid,
                line.project_id,
                line.head_id,
                version_id,
                line.testbed_id,
            );
            for (iteration, value) in values.iter().enumerate() {
                let uuid = self.uuid();
                let report_benchmark_id = create_report_benchmark_for_variant(
                    &mut self.conn,
                    &uuid,
                    report_id,
                    i32::try_from(iteration).expect("an iteration"),
                    line.benchmark_id,
                    line.variant_id,
                );
                let uuid = self.uuid();
                create_metric(
                    &mut self.conn,
                    &uuid,
                    report_benchmark_id,
                    line.measure_id,
                    *value,
                );
            }
            report_id
        }

        /// The next `count` versions, each a report of one metric at `level` with its noise.
        pub(crate) fn level(&mut self, level: f64, count: usize) {
            for _ in 0..count {
                let number = self.next;
                let version_id = self.version(number);
                self.report(version_id, &[level + NOISE[number % NOISE.len()]]);
                self.next += 1;
            }
        }

        /// One scan, as the task runs it: a read, then a write in its own transaction.
        fn scan(&mut self) -> bool {
            let watched = self.watched();
            self.scan_for(&watched)
        }

        fn scan_for(&mut self, watched: &WatchedLine) -> bool {
            let scan =
                Scan::read(&self.log, &mut self.conn, watched, &|| false).expect("Failed to scan");
            let Some(scan) = scan else {
                return false;
            };
            self.conn
                .immediate_transaction(|conn| scan.write(conn, NOW))
                .expect("Failed to write the scan")
        }

        /// The line's changepoints, oldest first, as (version number, status, scans).
        fn rows(&mut self) -> Vec<(usize, ChangepointState, usize)> {
            let rows = QueryChangepoint::on_line(&mut self.conn, &self.fixture.line)
                .expect("Failed to read the line");
            rows.into_iter()
                .map(|row| {
                    let number = self
                        .versions
                        .iter()
                        .position(|version_id| *version_id == row.version_id)
                        .expect("a version of the line");
                    let scans = QueryChangepoint::scans(&mut self.conn, row.id)
                        .expect("Failed to read the scans")
                        .len();
                    (number, row.status, scans)
                })
                .collect()
        }

        fn set_confirmations(&mut self, confirmations: i64) {
            diesel::update(
                schema::model::table.filter(schema::model::id.eq(self.fixture.model_id)),
            )
            .set(schema::model::confirmations.eq(confirmations))
            .execute(&mut self.conn)
            .expect("Failed to change the model");
        }

        pub(crate) fn set_model(
            &mut self,
            lower: Option<f64>,
            upper: Option<f64>,
            min: Option<i64>,
        ) {
            diesel::update(
                schema::model::table.filter(schema::model::id.eq(self.fixture.model_id)),
            )
            .set((
                schema::model::lower_boundary.eq(lower),
                schema::model::upper_boundary.eq(upper),
                schema::model::min_sample_size.eq(min),
            ))
            .execute(&mut self.conn)
            .expect("Failed to change the model");
        }

        pub(crate) fn into_conn(self) -> DbConnection {
            self.conn
        }

        /// Replace the threshold's changepoint model with a new one of the same settings, as an
        /// update of the threshold does, and return the new model.
        fn replace_model(&mut self) -> ModelId {
            let old = self.fixture.model_id;
            diesel::update(schema::model::table.filter(schema::model::id.eq(old)))
                .set(schema::model::replaced.eq(NOW))
                .execute(&mut self.conn)
                .expect("Failed to replace the model");
            let uuid = self.uuid();
            diesel::insert_into(schema::model::table)
                .values((
                    schema::model::uuid.eq(uuid),
                    schema::model::threshold_id.eq(self.fixture.threshold_id),
                    schema::model::test.eq(ModelTest::Changepoint),
                    schema::model::max_sample_size.eq(50),
                    schema::model::upper_boundary.eq(0.05),
                    schema::model::significance.eq(0.001),
                    schema::model::confirmations.eq(5),
                    schema::model::created.eq(NOW),
                ))
                .execute(&mut self.conn)
                .expect("Failed to insert a changepoint model");
            let model_id: ModelId = diesel::select(last_insert_rowid())
                .get_result(&mut self.conn)
                .expect("Failed to get the model id");
            diesel::update(
                schema::threshold::table
                    .filter(schema::threshold::id.eq(self.fixture.threshold_id)),
            )
            .set(schema::threshold::changepoint_model_id.eq(model_id))
            .execute(&mut self.conn)
            .expect("Failed to fill the changepoint slot");
            self.fixture.model_id = model_id;
            model_id
        }

        /// Replace the head as a branch reset does: the old head is marked replaced, and its
        /// changepoints are silenced.
        fn replace_head(&mut self) {
            let head_id: HeadId = self.branch.head_id;
            diesel::update(schema::head::table.filter(schema::head::id.eq(head_id)))
                .set(schema::head::replaced.eq(NOW))
                .execute(&mut self.conn)
                .expect("Failed to replace the head");
            QueryChangepoint::silence_all(&mut self.conn, head_id, NOW)
                .expect("Failed to silence the head");
        }
    }

    use ChangepointState::{Active, Candidate, Dismissed, Silenced, Withdrawn};

    /// The measured fields of the line's changepoint at `number`, to the bit.
    fn measured(line: &mut Line, number: usize) -> [Option<u64>; 6] {
        let row = row_at(line, number);
        [
            Some(row.mean_before),
            Some(row.mean_after),
            Some(row.stddev_before),
            Some(row.stddev_after),
            Some(row.pvalue),
            row.tstatistic,
        ]
        .map(|field| field.map(f64::to_bits))
    }

    /// What the newest scan of the line's changepoint at `number` measured, to the bit.
    fn last_scan(line: &mut Line, number: usize) -> [Option<u64>; 6] {
        let row = row_at(line, number);
        let scan = QueryChangepoint::scans(&mut line.conn, row.id)
            .expect("Failed to read the scans")
            .pop()
            .expect("a scan");
        [
            Some(scan.mean_before),
            Some(scan.mean_after),
            Some(scan.stddev_before),
            Some(scan.stddev_after),
            Some(scan.pvalue),
            scan.tstatistic,
        ]
        .map(|field| field.map(f64::to_bits))
    }

    /// The line's newest changepoint at `number`.
    fn row_at(line: &mut Line, number: usize) -> QueryChangepoint {
        let version_id = line.versions[number];
        QueryChangepoint::on_line(&mut line.conn, &line.fixture.line)
            .expect("Failed to read the line")
            .into_iter()
            .rfind(|row| row.version_id == version_id)
            .expect("a changepoint at the version")
    }

    /// A step confirmed active at version 20, from 100 to 110, after 5 scans with a new point
    /// before each scan after the first.
    fn confirmed_step() -> Line {
        let mut line = Line::new();
        line.level(100.0, 20);
        line.level(110.0, 5);
        line.scan();
        for _ in 0..4 {
            line.level(110.0, 1);
            line.scan();
        }
        assert_eq!(line.rows(), [(20, Active, 5)]);
        line
    }

    // Kills a candidate confirmed before its 5th scan or never, a scan that counts nothing or
    // records nothing, a candidate that keeps an earlier scan's numbers, a confirmation that keeps
    // the first scan's numbers, and a scan after it that rewrites them or records a scan.
    #[test]
    fn a_step_is_a_candidate_for_4_scans_and_active_at_the_5th() {
        let mut line = Line::new();
        line.level(100.0, 20);
        line.level(110.0, 10);

        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 1)]);
        let first = measured(&mut line, 20);
        for scans in 2..=4 {
            line.level(110.0, 1);
            assert!(line.scan());
            assert_eq!(line.rows(), [(20, Candidate, scans)], "scan {scans}");
            assert_eq!(
                measured(&mut line, 20),
                last_scan(&mut line, 20),
                "scan {scans}"
            );
        }
        line.level(110.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 5)]);
        let confirming = last_scan(&mut line, 20);
        assert_eq!(measured(&mut line, 20), confirming);
        assert_ne!(
            confirming, first,
            "each new point moves what a scan measures"
        );

        line.level(110.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 5)]);
        assert_eq!(measured(&mut line, 20), confirming);
    }

    // Kills a count of confirming scans that is not the model's: a model that takes 2 confirms at
    // the 2nd scan, and one that takes 1 at the first.
    #[test]
    fn a_candidate_is_confirmed_at_its_models_count() {
        let mut line = Line::new();
        line.set_confirmations(2);
        line.level(100.0, 20);
        line.level(110.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 1)]);
        line.level(110.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 2)]);
        assert_eq!(measured(&mut line, 20), last_scan(&mut line, 20));

        let mut line = Line::new();
        line.set_confirmations(1);
        line.level(100.0, 20);
        line.level(110.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 1)]);
        assert_eq!(measured(&mut line, 20), last_scan(&mut line, 20));
    }

    // Kills a newest point that is not found, or that is shown at once.
    #[test]
    fn a_newest_point_that_jumps_is_held_as_a_candidate() {
        let mut line = Line::new();
        line.level(100.0, 30);
        let newest = line.version(30);
        line.report(newest, &[120.0]);

        assert!(line.scan());
        assert_eq!(line.rows(), [(30, Candidate, 1)]);
        let row = QueryChangepoint::on_line(&mut line.conn, &line.fixture.line)
            .expect("Failed to read the line")
            .remove(0);
        assert!(
            QueryChangepoint::from_uuid(&mut line.conn, row.project_id, row.uuid).is_err(),
            "a candidate is never shown"
        );
    }

    // Kills a candidate that a scan without it keeps, or withdraws in place of deleting.
    #[test]
    fn a_candidate_that_vanishes_before_confirmation_leaves_no_row() {
        let mut line = Line::new();
        line.level(100.0, 30);
        let newest = line.version(30);
        line.report(newest, &[120.0]);
        assert!(line.scan());
        assert_eq!(line.rows(), [(30, Candidate, 1)]);

        // A second run of the newest commit brings its median back to the line.
        line.report(newest, &[100.2, 99.8]);
        assert!(line.scan());
        assert_eq!(line.rows(), []);
    }

    // Kills an active changepoint that stays when its step moves, and a moved step that is
    // confirmed at once or not found.
    #[test]
    fn a_confirmed_step_that_moves_is_withdrawn_and_its_new_version_is_a_candidate() {
        let mut line = confirmed_step();
        let step = line.versions[20];

        // A second run of the commit at the step brings its median down, so the step moves to the
        // next version.
        line.report(step, &[99.9, 100.1]);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Withdrawn, 5), (21, Candidate, 1)]);
    }

    // Kills an active changepoint that stays when its step vanishes.
    #[test]
    fn a_confirmed_step_that_vanishes_is_withdrawn() {
        let mut line = confirmed_step();

        // Second runs of every commit after the step bring each median back down.
        for number in 20..line.next {
            let version_id = line.versions[number];
            line.report(version_id, &[99.9, 100.1]);
        }
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Withdrawn, 5)]);
    }

    // Kills a withdrawn changepoint that holds its version against a step that returns to it.
    #[test]
    fn a_confirmed_step_that_moves_away_and_returns_is_a_new_candidate() {
        let mut line = confirmed_step();
        let step = line.versions[20];
        line.report(step, &[99.9, 100.1]);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Withdrawn, 5), (21, Candidate, 1)]);

        // 2 more runs of the commit at the step bring its median back up, so the step returns.
        line.report(step, &[110.1, 109.9]);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Withdrawn, 5), (20, Candidate, 1)]);
    }

    // Kills a dismissed or silenced changepoint that no longer holds its version, so a step a
    // user dismissed would come back.
    #[test]
    fn a_dismissed_or_silenced_step_found_again_gets_no_candidate() {
        for state in [Dismissed, Silenced] {
            let mut line = confirmed_step();
            let row = row_at(&mut line, 20);
            diesel::update(schema::changepoint::table.filter(schema::changepoint::id.eq(row.id)))
                .set(schema::changepoint::status.eq(state))
                .execute(&mut line.conn)
                .expect("Failed to set the status");
            line.level(110.0, 1);
            assert!(line.scan());
            assert_eq!(line.rows(), [(20, state, 5)], "{state:?}");
        }
    }

    // Kills a line that reads the versions its head cloned from its start point.
    #[test]
    fn a_step_below_the_start_point_is_not_seen_and_one_above_it_is() {
        let mut line = Line::new();
        line.level(100.0, 15);
        line.level(110.0, 15);

        // A branch that starts from main at its newest version, as a pull request does, with
        // main's versions cloned onto its head and its own versions numbered above them.
        let start_point = line.versions[29];
        let pr = create_branch_with_head(
            &mut line.conn,
            line.fixture.line.project_id,
            "00000000-0000-0000-0000-000000000020",
            "pr",
            "pr",
            "00000000-0000-0000-0000-000000000021",
        );
        let mut start_point_id = None;
        for version_id in line.versions.clone() {
            let head_version_id = create_head_version(&mut line.conn, pr.head_id, version_id);
            if version_id == start_point {
                start_point_id = Some(head_version_id);
            }
        }
        diesel::update(schema::head::table.filter(schema::head::id.eq(pr.head_id)))
            .set(schema::head::start_point_id.eq(start_point_id))
            .execute(&mut line.conn)
            .expect("Failed to set the start point");
        line.branch = pr;
        line.fixture.line.head_id = pr.head_id;

        line.level(110.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), []);

        line.level(121.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), [(40, Candidate, 1)]);
    }

    // Kills a scan of a replaced head, from the queue or from a replacement between the read and
    // the write, which would undo the silence or leave a candidate no scan ever removes.
    #[test]
    fn a_head_replaced_while_its_line_is_queued_is_skipped() {
        let mut line = confirmed_step();
        line.level(121.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 5), (29, Candidate, 1)]);

        line.replace_head();
        assert_eq!(line.rows(), [(20, Silenced, 5)]);
        assert!(!line.scan());
        assert_eq!(line.rows(), [(20, Silenced, 5)]);

        let mut line = Line::new();
        line.level(100.0, 20);
        line.level(110.0, 10);
        let watched = line.watched();
        let scan = Scan::read(&line.log, &mut line.conn, &watched, &|| false)
            .expect("Failed to scan")
            .expect("a scan of the line");
        line.replace_head();
        let written = line
            .conn
            .immediate_transaction(|conn| scan.write(conn, NOW))
            .expect("Failed to write the scan");
        assert!(!written);
        assert_eq!(line.rows(), []);
    }

    // Kills points ordered by report and a result that keeps a rerun's old point.
    #[test]
    fn a_rerun_of_an_old_commit_moves_the_step_with_its_point() {
        let mut line = Line::new();
        line.level(100.0, 11);
        line.level(110.0, 19);
        assert!(line.scan());
        assert_eq!(line.rows(), [(11, Candidate, 1)]);

        // The commit at version 10 runs again after every other report.
        let rerun = line.versions[10];
        line.report(rerun, &[110.2, 109.8]);
        assert!(line.scan());
        assert_eq!(line.rows(), [(10, Candidate, 1)]);
    }

    // Kills a scan that reconciles the changepoints of another threshold on the same line.
    #[test]
    fn a_scan_reconciles_only_its_own_thresholds_changepoints() {
        let mut line = Line::new();
        line.level(100.0, 20);
        line.level(110.0, 10);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 1)]);

        // A second threshold watches the same line through a filter that matches every variant,
        // with a changepoint model that guards decreases only.
        let other = create_threshold_with_dimensions(
            &mut line.conn,
            line.fixture.line.project_id,
            line.branch.branch_id,
            line.fixture.line.testbed_id,
            Some(ParameterFilter::new(vec![ParameterSet::default()])),
            line.fixture.line.measure_id,
            None,
            "00000000-0000-0000-0000-000000000040",
        );
        diesel::insert_into(schema::model::table)
            .values((
                schema::model::uuid.eq("00000000-0000-0000-0000-000000000041"),
                schema::model::threshold_id.eq(other),
                schema::model::test.eq(ModelTest::Changepoint),
                schema::model::max_sample_size.eq(50),
                schema::model::lower_boundary.eq(0.05),
                schema::model::significance.eq(0.001),
                schema::model::confirmations.eq(5),
                schema::model::created.eq(NOW),
            ))
            .execute(&mut line.conn)
            .expect("Failed to insert a changepoint model");
        let model_id: ModelId = diesel::select(last_insert_rowid())
            .get_result(&mut line.conn)
            .expect("Failed to get the model id");
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(other)))
            .set(schema::threshold::changepoint_model_id.eq(model_id))
            .execute(&mut line.conn)
            .expect("Failed to fill the changepoint slot");
        let watched = WatchedLine {
            line: line.fixture.line.clone(),
            threshold_id: other,
        };

        assert!(line.scan_for(&watched));
        assert_eq!(line.rows(), [(20, Candidate, 1)]);
        line.level(110.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 2)]);
    }

    // Kills a candidate that keeps its count under a model the threshold no longer holds, and an
    // active changepoint that a new model takes back.
    #[test]
    fn a_candidate_under_a_replaced_model_starts_again_under_the_new_one() {
        let mut line = confirmed_step();
        let old_model = line.fixture.model_id;
        line.level(121.0, 10);
        assert!(line.scan());
        for _ in 0..2 {
            line.level(121.0, 1);
            assert!(line.scan());
        }
        assert_eq!(line.rows(), [(20, Active, 5), (29, Candidate, 3)]);

        let new_model = line.replace_model();
        line.level(121.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Active, 5), (29, Candidate, 1)]);
        assert_eq!(row_at(&mut line, 20).model_id, old_model);
        assert_eq!(row_at(&mut line, 29).model_id, new_model);
    }

    // Kills a scan that writes under a model the threshold no longer holds. A threshold update that
    // empties the changepoint slot during the analysis would leave a candidate, or at 1
    // confirmation an active changepoint, that no scan reaches and that refuses the threshold's
    // delete; one that replaces the slot would leave a row under the old model; a threshold
    // deleted during the analysis would fail the write; and a threshold deleted and made again
    // with the same ids would take a row under a model the scan never read.
    #[test]
    fn a_scan_writes_nothing_under_a_model_its_threshold_no_longer_holds() {
        let outcomes = [
            during_the_analysis(5, empty_the_slot),
            during_the_analysis(1, empty_the_slot),
            during_the_analysis(5, replace_the_slot),
            during_the_analysis(5, delete_the_threshold),
            during_the_analysis(5, make_the_threshold_again),
        ];
        let unwritten = || Outcome {
            written: false,
            rows: Vec::new(),
            deleted: true,
        };
        assert_eq!(
            outcomes,
            [
                unwritten(),
                unwritten(),
                unwritten(),
                unwritten(),
                unwritten()
            ]
        );
    }

    #[derive(Debug, PartialEq)]
    struct Outcome {
        written: bool,
        rows: Vec<(usize, ChangepointState, usize)>,
        /// Whether the threshold deletes after the scan.
        deleted: bool,
    }

    /// Read a scan of a line with a step under a model of `confirmations`, write the threshold's
    /// changepoint slot with `update` in its own transaction, then write the scan.
    fn during_the_analysis(
        confirmations: i64,
        update: fn(&QueryThreshold, &mut DbConnection) -> diesel::QueryResult<()>,
    ) -> Outcome {
        let mut line = Line::new();
        line.set_confirmations(confirmations);
        line.level(100.0, 20);
        line.level(110.0, 10);
        let watched = line.watched();
        let scan = Scan::read(&line.log, &mut line.conn, &watched, &|| false)
            .expect("Failed to scan")
            .expect("a scan of the line");
        let threshold = schema::threshold::table
            .filter(schema::threshold::id.eq(watched.threshold_id))
            .first::<QueryThreshold>(&mut line.conn)
            .expect("Failed to get the threshold");
        line.conn
            .immediate_transaction(|conn| update(&threshold, conn))
            .expect("Failed to update the threshold");
        let written = line
            .conn
            .immediate_transaction(|conn| scan.write(conn, NOW))
            .expect("Failed to write the scan");
        let rows = line.rows();
        let deleted = diesel::delete(
            schema::threshold::table.filter(schema::threshold::id.eq(watched.threshold_id)),
        )
        .execute(&mut line.conn)
        .is_ok();
        Outcome {
            written,
            rows,
            deleted,
        }
    }

    /// Empty the changepoint slot, as a threshold update that removes the model does.
    fn empty_the_slot(
        threshold: &QueryThreshold,
        conn: &mut DbConnection,
    ) -> diesel::QueryResult<()> {
        threshold.remove_slot_model(conn, ModelSlot::ChangepointModel)
    }

    /// Fill the changepoint slot with a new model, as a threshold update that changes it does.
    fn replace_the_slot(
        threshold: &QueryThreshold,
        conn: &mut DbConnection,
    ) -> diesel::QueryResult<()> {
        let model_id = threshold
            .changepoint_model_id
            .ok_or(diesel::result::Error::NotFound)?;
        let model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .select(QueryModel::as_select())
            .first::<QueryModel>(conn)?
            .into_model();
        threshold.update_slot_inner(conn, ModelSlot::ChangepointModel, model)
    }

    fn delete_the_threshold(
        threshold: &QueryThreshold,
        conn: &mut DbConnection,
    ) -> diesel::QueryResult<()> {
        diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(threshold.id)))
            .execute(conn)
            .map(drop)
    }

    /// Delete the threshold, which takes its model, then make a threshold on the same dimensions
    /// with a changepoint model of the same settings. SQLite gives a new row the id after the
    /// highest, and the deleted threshold and model held the highest ids, so the new ones get
    /// theirs.
    fn make_the_threshold_again(
        threshold: &QueryThreshold,
        conn: &mut DbConnection,
    ) -> diesel::QueryResult<()> {
        let model_id = threshold
            .changepoint_model_id
            .ok_or(diesel::result::Error::NotFound)?;
        let model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .select(QueryModel::as_select())
            .first::<QueryModel>(conn)?
            .into_model();
        delete_the_threshold(threshold, conn)?;
        let threshold_id = create_threshold_with_dimensions(
            conn,
            threshold.project_id,
            threshold.branch_id,
            threshold.testbed_id,
            None,
            threshold.measure_id,
            None,
            "00000000-0000-0000-0000-000000000050",
        );
        let again = schema::threshold::table
            .filter(schema::threshold::id.eq(threshold_id))
            .first::<QueryThreshold>(conn)?;
        again.update_slot_inner(conn, ModelSlot::ChangepointModel, model)?;
        let again_model_id = schema::threshold::table
            .filter(schema::threshold::id.eq(threshold_id))
            .select(schema::threshold::changepoint_model_id)
            .first::<Option<ModelId>>(conn)?;
        assert_eq!(
            (threshold_id, again_model_id),
            (threshold.id, Some(model_id)),
            "the threshold and model made again have the deleted ones' ids"
        );
        Ok(())
    }

    /// The side of the line's changepoint at `number`: whether its mean went up.
    fn went_up(line: &mut Line, number: usize) -> bool {
        let row = row_at(line, number);
        row.mean_after > row.mean_before
    }

    // Kills one pass for both sides, a pass with the other side's boundary, a pass that keeps the
    // other direction, and a side with no boundary that runs a pass.
    #[test]
    fn each_guarded_side_keeps_its_own_direction_past_its_own_boundary() {
        let mut both = Line::new();
        both.set_model(Some(0.05), Some(0.05), None);
        both.level(100.0, 20);
        both.level(110.0, 20);
        both.level(99.0, 20);
        assert!(both.scan());
        assert_eq!(both.rows(), [(20, Candidate, 1), (40, Candidate, 1)]);
        assert!(went_up(&mut both, 20));
        assert!(!went_up(&mut both, 40));

        // A decrease of about a tenth does not pass a lower boundary of a fifth.
        let mut wide_lower = Line::new();
        wide_lower.set_model(Some(0.2), Some(0.05), None);
        wide_lower.level(100.0, 20);
        wide_lower.level(110.0, 20);
        wide_lower.level(99.0, 20);
        assert!(wide_lower.scan());
        assert_eq!(wide_lower.rows(), [(20, Candidate, 1)]);

        let mut upper_only = Line::new();
        upper_only.level(100.0, 20);
        upper_only.level(90.0, 20);
        assert!(upper_only.scan());
        assert_eq!(upper_only.rows(), []);

        let mut lower_only = Line::new();
        lower_only.set_model(Some(0.05), None, None);
        lower_only.level(100.0, 20);
        lower_only.level(90.0, 20);
        assert!(lower_only.scan());
        assert_eq!(lower_only.rows(), [(20, Candidate, 1)]);
        assert!(!went_up(&mut lower_only, 20));
    }

    fn spec(line: &mut Line, label: &str, cpu: i32) -> SpecId {
        create_spec(
            &mut line.conn,
            CreateSpecArgs {
                uuid: &format!("00000000-0000-0000-0000-0000000005{label}0"),
                name: label,
                slug: label,
                os: "linux",
                architecture: "x86_64",
                cpu,
                memory: 0x0004_0000_0000,
                disk: 0x0019_0000_0000,
                network: false,
            },
        )
    }

    // Kills a point that is a mean, the first or the last metric, a point per report, a second
    // report at a version left out, a report under another spec or another name let in, and a
    // version with no metric on the line given a point.
    #[test]
    fn a_point_is_the_median_of_the_lines_metrics_on_its_version() {
        let mut line = Line::new();
        let spec_a = spec(&mut line, "a", 4);
        let spec_b = spec(&mut line, "b", 8);
        line.fixture.line.spec_id = Some(spec_a);
        let under = |line: &mut Line, spec_id: Option<SpecId>, version, values: &[f64]| {
            let report_id = line.report(version, values);
            if let Some(spec_id) = spec_id {
                set_report_spec(&mut line.conn, report_id, spec_id);
            }
            report_id
        };

        // 3 iterations, one an outlier.
        let v0 = line.version(0);
        under(&mut line, Some(spec_a), v0, &[100.0, 101.0, 500.0]);
        // A second report at the version, as a control report is, counts; a report under another
        // spec, or under none, does not.
        let v1 = line.version(1);
        let first = under(&mut line, Some(spec_a), v1, &[100.0]);
        under(&mut line, Some(spec_a), v1, &[104.0, 102.0]);
        under(&mut line, Some(spec_b), v1, &[900.0]);
        under(&mut line, None, v1, &[800.0]);
        // Another name of the measure is another line.
        let uuid = line.uuid();
        let report_benchmark_id = schema::report_benchmark::table
            .filter(schema::report_benchmark::report_id.eq(first))
            .select(schema::report_benchmark::id)
            .first(&mut line.conn)
            .expect("a report benchmark");
        create_named_metric(
            &mut line.conn,
            &uuid,
            report_benchmark_id,
            line.fixture.line.measure_id,
            &"p99".parse().expect("a metric name"),
            700.0,
        );
        // Version 2 holds a metric of another benchmark only.
        let v2 = line.version(2);
        let other = create_benchmark(
            &mut line.conn,
            line.fixture.line.project_id,
            "00000000-0000-0000-0000-000000000030",
            "other",
            "other",
        );
        let other_variant = get_empty_variant(&mut line.conn, other);
        let report_id = under(&mut line, Some(spec_a), v2, &[]);
        let uuid = line.uuid();
        let report_benchmark_id = create_report_benchmark_for_variant(
            &mut line.conn,
            &uuid,
            report_id,
            0,
            other,
            other_variant,
        );
        let uuid = line.uuid();
        create_metric(
            &mut line.conn,
            &uuid,
            report_benchmark_id,
            line.fixture.line.measure_id,
            300.0,
        );
        // An even count takes the mean of the middle two.
        let v3 = line.version(3);
        under(&mut line, Some(spec_a), v3, &[99.0, 103.0]);

        let points = line_points(&mut line.conn, &line.fixture.line).expect("Failed to read");
        assert_eq!(
            points,
            [
                LinePoint {
                    version_id: v0,
                    value: 101.0
                },
                LinePoint {
                    version_id: v1,
                    value: 102.0
                },
                LinePoint {
                    version_id: v3,
                    value: 101.0
                },
            ]
        );
    }

    // Kills a line with no spec that reads a report under a spec.
    #[test]
    fn a_line_with_no_spec_reads_only_reports_with_no_spec() {
        let mut line = Line::new();
        let spec_a = spec(&mut line, "a", 4);
        assert_eq!(line.fixture.line.spec_id, None);
        let v0 = line.version(0);
        line.report(v0, &[100.0]);
        let under_a = line.report(v0, &[900.0]);
        set_report_spec(&mut line.conn, under_a, spec_a);
        let v1 = line.version(1);
        let only_under_a = line.report(v1, &[800.0]);
        set_report_spec(&mut line.conn, only_under_a, spec_a);

        let points = line_points(&mut line.conn, &line.fixture.line).expect("Failed to read");
        assert_eq!(
            points,
            [LinePoint {
                version_id: v0,
                value: 100.0
            }]
        );
    }

    // Kills a line that reads a version its head does not hold, as another branch's version of the
    // line's variant is, and one that reads a version its head cloned at or below its start point.
    #[test]
    fn a_line_reads_only_the_versions_native_to_its_head() {
        let mut line = Line::new();
        let main_line = line.fixture.line.clone();
        let main: Vec<VersionId> = (0..3).map(|number| line.version(number)).collect();
        for (version_id, value) in main.iter().zip([1.0, 2.0, 3.0]) {
            line.report(*version_id, &[value]);
        }

        // A branch that starts from main at its version 1, so main's version 2 is not on it, and
        // its own versions are numbered from 2.
        let pr = create_branch_with_head(
            &mut line.conn,
            main_line.project_id,
            "00000000-0000-0000-0000-000000000020",
            "pr",
            "pr",
            "00000000-0000-0000-0000-000000000021",
        );
        let mut start_point_id = None;
        for version_id in &main[..2] {
            start_point_id = Some(create_head_version(&mut line.conn, pr.head_id, *version_id));
        }
        diesel::update(schema::head::table.filter(schema::head::id.eq(pr.head_id)))
            .set(schema::head::start_point_id.eq(start_point_id))
            .execute(&mut line.conn)
            .expect("Failed to set the start point");
        line.branch = pr;
        line.fixture.line.head_id = pr.head_id;
        line.versions.truncate(2);
        let pr_2 = line.version(2);
        let pr_3 = line.version(3);
        line.report(pr_2, &[20.0]);
        line.report(pr_3, &[30.0]);

        let points = line_points(&mut line.conn, &line.fixture.line).expect("Failed to read");
        assert_eq!(
            points,
            [
                LinePoint {
                    version_id: pr_2,
                    value: 20.0
                },
                LinePoint {
                    version_id: pr_3,
                    value: 30.0
                },
            ]
        );
        let points = line_points(&mut line.conn, &main_line).expect("Failed to read");
        assert_eq!(
            points,
            [
                LinePoint {
                    version_id: main[0],
                    value: 1.0
                },
                LinePoint {
                    version_id: main[1],
                    value: 2.0
                },
                LinePoint {
                    version_id: main[2],
                    value: 3.0
                },
            ]
        );
    }

    // Kills an order that reads other rows than the other order does: a filter on the spec, the
    // testbed, the head, the start point, the variant, the measure, or the metric name that one
    // order drops or changes, and an order that keeps one metric of a version, not the median.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture of 2 heads, read as each line in each order"
    )]
    fn both_orders_of_a_line_read_the_same_points() {
        let mut line = Line::new();
        let spec_a = spec(&mut line, "a", 4);
        let spec_b = spec(&mut line, "b", 8);
        let other_testbed = create_testbed(
            &mut line.conn,
            line.fixture.line.project_id,
            "00000000-0000-0000-0000-000000000040",
            "other",
            "other",
        );
        let other_variant = create_variant(
            &mut line.conn,
            line.fixture.line.benchmark_id,
            &r#"{"size": 2}"#.parse().expect("a set"),
        );
        let other_measure = create_measure(
            &mut line.conn,
            line.fixture.line.project_id,
            "00000000-0000-0000-0000-000000000050",
            "other",
            "other",
        );
        let other_name: MetricName = "p99".parse().expect("a metric name");
        // A report of one iteration for each value on the line, beside a metric of another variant
        // of the benchmark, one of another measure under the line's name, and one of the line's
        // measure under another name. Each neighbor is above every value on the line, so any one
        // of them moves the median.
        let own =
            |line: &mut Line, version_id: VersionId, spec_id: Option<SpecId>, values: &[f64]| {
                let report_id = line.report(version_id, values);
                if let Some(spec_id) = spec_id {
                    set_report_spec(&mut line.conn, report_id, spec_id);
                }
                let ChangepointLine {
                    benchmark_id,
                    measure_id,
                    metric,
                    ..
                } = line.fixture.line.clone();
                let report_benchmark_id = schema::report_benchmark::table
                    .filter(schema::report_benchmark::report_id.eq(report_id))
                    .select(schema::report_benchmark::id)
                    .first(&mut line.conn)
                    .expect("a report benchmark");
                let uuid = line.uuid();
                let other_variant_benchmark_id = create_report_benchmark_for_variant(
                    &mut line.conn,
                    &uuid,
                    report_id,
                    0,
                    benchmark_id,
                    other_variant,
                );
                for (report_benchmark_id, measure_id, name, value) in [
                    (other_variant_benchmark_id, measure_id, &metric, 600.0),
                    (report_benchmark_id, other_measure, &metric, 500.0),
                    (report_benchmark_id, measure_id, &other_name, 400.0),
                ] {
                    let uuid = line.uuid();
                    create_named_metric(
                        &mut line.conn,
                        &uuid,
                        report_benchmark_id,
                        measure_id,
                        name,
                        value,
                    );
                }
            };
        // Each version holds 2 reports of 2 iterations under spec a and 2 under no spec, one under
        // spec b, and one on another testbed. The point is the median of the 4 values. No one
        // value is the point, and no pair of 1 value from each report has the point as its median.
        let reports = |line: &mut Line, version_id: VersionId, a: f64, none: f64| {
            for (spec_id, point) in [(Some(spec_a), a), (None, none)] {
                own(line, version_id, spec_id, &[point - 1.0, point + 1.0]);
                own(line, version_id, spec_id, &[point - 5.0, point + 7.0]);
            }
            let under_b = line.report(version_id, &[900.0]);
            set_report_spec(&mut line.conn, under_b, spec_b);
            let elsewhere = line.report(version_id, &[700.0]);
            diesel::update(schema::report::table.filter(schema::report::id.eq(elsewhere)))
                .set(schema::report::testbed_id.eq(other_testbed))
                .execute(&mut line.conn)
                .expect("Failed to move the report");
        };
        let main_line = line.fixture.line.clone();
        let main: Vec<VersionId> = (0..4).map(|number| line.version(number)).collect();
        for (version_id, a) in main.iter().zip([1.0, 2.0, 3.0, 4.0]) {
            reports(&mut line, *version_id, a, a + 10.0);
        }

        // A branch with a start point at main's version 1, and versions of its own from 2.
        let pr = create_branch_with_head(
            &mut line.conn,
            main_line.project_id,
            "00000000-0000-0000-0000-000000000020",
            "pr",
            "pr",
            "00000000-0000-0000-0000-000000000021",
        );
        let mut start_point_id = None;
        for version_id in &main[..2] {
            start_point_id = Some(create_head_version(&mut line.conn, pr.head_id, *version_id));
        }
        diesel::update(schema::head::table.filter(schema::head::id.eq(pr.head_id)))
            .set(schema::head::start_point_id.eq(start_point_id))
            .execute(&mut line.conn)
            .expect("Failed to set the start point");
        line.branch = pr;
        line.fixture.line.head_id = pr.head_id;
        line.versions.truncate(2);
        let pr_versions: Vec<VersionId> = (2..4).map(|number| line.version(number)).collect();
        for (version_id, a) in pr_versions.iter().zip([20.0, 30.0]) {
            reports(&mut line, *version_id, a, a + 1.0);
        }
        let pr_line = line.fixture.line.clone();

        let points = |versions: &[VersionId], values: &[f64]| -> Vec<LinePoint> {
            versions
                .iter()
                .zip(values)
                .map(|(version_id, value)| LinePoint {
                    version_id: *version_id,
                    value: *value,
                })
                .collect()
        };
        let lines = [
            (
                "main under spec a",
                ChangepointLine {
                    spec_id: Some(spec_a),
                    ..main_line.clone()
                },
                points(&main, &[1.0, 2.0, 3.0, 4.0]),
            ),
            (
                "main under no spec",
                main_line,
                points(&main, &[11.0, 12.0, 13.0, 14.0]),
            ),
            (
                "pr under spec a",
                ChangepointLine {
                    spec_id: Some(spec_a),
                    ..pr_line.clone()
                },
                points(&pr_versions, &[20.0, 30.0]),
            ),
            (
                "pr under no spec",
                pr_line,
                points(&pr_versions, &[21.0, 31.0]),
            ),
        ];
        // Every line in each order, compared at once, so a difference shows each line and order
        // it reaches.
        let mut read = Vec::new();
        let mut expected = Vec::new();
        for (name, changepoint_line, points) in lines {
            let start_point = start_point_of(&mut line.conn, changepoint_line.head_id)
                .expect("Failed to read the start point");
            for order in [LineOrder::HeadFirst, LineOrder::VariantFirst] {
                let order_points =
                    line_points_in_order(&mut line.conn, &changepoint_line, start_point, order)
                        .expect("Failed to read");
                read.push((name, format!("{order:?}"), order_points));
                expected.push((name, format!("{order:?}"), points.clone()));
            }
        }
        assert_eq!(read, expected);
    }

    // Kills a changepoint that cannot be stored kept for an insert, and a NaN mean that is dropped
    // by its side, which it has none of, before it is logged.
    #[test]
    fn a_changepoint_that_cannot_be_stored_is_never_kept_and_always_logged() {
        let capture = Capture::default();
        let log = Logger::root(capture.clone().fuse(), slog::o!());
        let line = Line::new();
        let points = [LinePoint {
            version_id: line.fixture.version_id,
            value: 100.0,
        }];
        let at = |changepoint: Changepoint| Changepoint {
            index: 0,
            ..changepoint
        };
        let up = at(MEASURED_STEP);
        let down = at(Changepoint {
            mean_before: 110.0,
            mean_after: 100.0,
            ..MEASURED_STEP
        });
        // An increase with a deviation that cannot be stored, and means with no side.
        let unstorable_up = at(Changepoint {
            stddev_before: f64::NAN,
            ..MEASURED_STEP
        });
        let nan_mean = at(Changepoint {
            mean_after: f64::NAN,
            ..MEASURED_STEP
        });
        let found = kept_on_side(
            &log,
            &line.fixture.line,
            &points,
            Side::Upper,
            vec![unstorable_up, nan_mean, down, up],
        );
        assert_eq!(
            found
                .iter()
                .map(|found| (found.side, found.version_id, found.changepoint.mean_after))
                .collect::<Vec<_>>(),
            [(Side::Upper, line.fixture.version_id, 110.0)]
        );
        let lines = capture.0.lock().expect("a log").clone();
        for field in ["stddev_before", "mean_after"] {
            assert!(
                lines.iter().any(|line| line.starts_with("Refused")
                    && line.contains(&format!(" {field}="))),
                "{field}: {lines:?}"
            );
        }
        assert_eq!(lines.len(), 2, "{lines:?}");
    }

    // Kills a threshold whose parameters filter leaves a variant out that still watches it, and one
    // that watches a metric the report did not name.
    #[test]
    fn a_report_watches_only_the_variants_and_metrics_a_threshold_watches() {
        let set = |json: &str| -> ParameterSet { json.parse().expect("a set") };
        let filter = |json: &str| ParameterFilter::new(vec![set(json)]);
        let watch =
            |threshold: i32, parameters: Option<ParameterFilter>, metric: &str| ChangepointWatch {
                threshold_id: super::ThresholdId::from_raw(threshold),
                parameters,
                metric: metric.parse().expect("a metric name"),
            };
        let watches = [
            watch(1, None, "value"),
            watch(2, Some(filter(r#"{"size": 1}"#)), "value"),
            watch(3, Some(filter(r#"{"size": 2}"#)), "value"),
            watch(4, None, "p99"),
        ];
        let named = std::collections::BTreeMap::from([(MetricName::value(), 1.0)]);
        let watching = |variant: &ParameterSet| {
            ChangepointWatch::watching(&watches, variant, &named)
                .map(|watch| watch.threshold_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            watching(&set(r#"{"size": 1}"#)),
            [1, 2].map(super::ThresholdId::from_raw)
        );
        assert_eq!(
            watching(&set(r#"{"size": 2}"#)),
            [1, 3].map(super::ThresholdId::from_raw)
        );
    }

    // Kills a scan that goes on to a pass once stopped, that never checks, that checks only before
    // one of its passes, or that checks after its last pass.
    #[test]
    fn a_scan_stops_before_each_pass_once_stopped() {
        let mut line = Line::new();
        line.set_model(Some(0.05), Some(0.05), None);
        line.level(100.0, 20);
        line.level(110.0, 10);
        let watched = line.watched();
        // Whether the scan has a result, and how many times it checked, when it is stopped from
        // the `stop_at`th check on.
        let read = |line: &mut Line, stop_at: usize| {
            let checks = Cell::new(0usize);
            let stopped = || {
                checks.set(checks.get().saturating_add(1));
                checks.get() >= stop_at
            };
            let scan =
                Scan::read(&line.log, &mut line.conn, &watched, &stopped).expect("Failed to scan");
            (scan.is_some(), checks.get())
        };
        assert_eq!(
            [1, 2, usize::MAX].map(|stop_at| read(&mut line, stop_at)),
            [(false, 1), (false, 2), (true, 2)]
        );

        line.set_model(None, Some(0.05), None);
        assert_eq!(read(&mut line, usize::MAX), (true, 1));
    }

    // Kills a minimum that is not checked, or checked against the window or the metrics.
    #[test]
    fn a_line_shorter_than_the_minimum_is_not_analysed() {
        let mut line = Line::new();
        line.set_model(None, Some(0.05), Some(31));
        line.level(100.0, 20);
        line.level(110.0, 10);
        assert!(!line.scan());
        assert_eq!(line.rows(), []);

        line.level(110.0, 1);
        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 1)]);
    }

    // Kills a step between 2 constant sides that is refused for its infinite t statistic, or
    // stored with one.
    #[test]
    fn a_step_between_2_constant_sides_is_confirmed_with_no_t_statistic() {
        let mut line = Line::new();
        for number in 0..40 {
            let version_id = line.version(number);
            line.report(version_id, &[if number < 20 { 100.0 } else { 110.0 }]);
        }

        assert!(line.scan());
        assert_eq!(line.rows(), [(20, Candidate, 1)]);
        for number in 40..44 {
            let version_id = line.version(number);
            line.report(version_id, &[110.0]);
            assert!(line.scan());
        }
        assert_eq!(line.rows(), [(20, Active, 5)]);
        let row = row_at(&mut line, 20);
        assert_eq!(row.tstatistic, None);
        assert_eq!((row.mean_before, row.mean_after), (100.0, 110.0));
    }

    // Kills a NaN, or a mean, standard deviation, or p value that is not finite, that is let
    // through; a refusal with no log line; and an infinite t statistic that is refused.
    #[test]
    fn a_nan_or_a_non_finite_mean_deviation_or_p_value_is_refused_and_logged() {
        let capture = Capture::default();
        let log = Logger::root(capture.clone().fuse(), slog::o!());
        let line = Line::new();
        let cases = [
            (
                "mean_before",
                Changepoint {
                    mean_before: f64::INFINITY,
                    ..MEASURED_STEP
                },
            ),
            (
                "mean_after",
                Changepoint {
                    mean_after: f64::NAN,
                    ..MEASURED_STEP
                },
            ),
            (
                "stddev_before",
                Changepoint {
                    stddev_before: f64::NAN,
                    ..MEASURED_STEP
                },
            ),
            (
                "stddev_after",
                Changepoint {
                    stddev_after: f64::INFINITY,
                    ..MEASURED_STEP
                },
            ),
            (
                "pvalue",
                Changepoint {
                    pvalue: f64::NAN,
                    ..MEASURED_STEP
                },
            ),
            (
                "tstatistic",
                Changepoint {
                    tstatistic: f64::NAN,
                    ..MEASURED_STEP
                },
            ),
        ];
        for (field, changepoint) in cases {
            capture.0.lock().expect("a log").clear();
            assert!(
                !is_storable(
                    &log,
                    &line.fixture.line,
                    line.fixture.version_id,
                    &changepoint
                ),
                "{field}"
            );
            let lines = capture.0.lock().expect("a log").clone();
            assert!(
                lines
                    .iter()
                    .any(|line| line.starts_with("Refused") && line.contains(&format!(" {field}="))),
                "{field}: {lines:?}"
            );
        }

        for tstatistic in [f64::NEG_INFINITY, f64::INFINITY, MEASURED_STEP.tstatistic] {
            capture.0.lock().expect("a log").clear();
            let changepoint = Changepoint {
                tstatistic,
                ..MEASURED_STEP
            };
            assert!(
                is_storable(
                    &log,
                    &line.fixture.line,
                    line.fixture.version_id,
                    &changepoint
                ),
                "{tstatistic}"
            );
            assert_eq!(*capture.0.lock().expect("a log"), Vec::<String>::new());
        }
    }

    /// Keeps the message of every log line, with its key-value pairs.
    #[derive(Clone, Default)]
    pub(crate) struct Capture(Arc<Mutex<Vec<String>>>);

    struct LogLine(String);

    impl Drain for Capture {
        type Ok = ();
        type Err = slog::Never;

        fn log(&self, record: &Record<'_>, values: &OwnedKVList) -> Result<(), slog::Never> {
            self.keep(record, values);
            Ok(())
        }
    }

    impl Capture {
        pub(crate) fn lines(&self) -> Vec<String> {
            self.0.lock().expect("a log").clone()
        }

        fn keep(&self, record: &Record<'_>, values: &OwnedKVList) {
            let mut line = LogLine(record.msg().to_string());
            record
                .kv()
                .serialize(record, &mut line)
                .expect("a serialized line");
            values
                .serialize(record, &mut line)
                .expect("a serialized line");
            self.0.lock().expect("a log").push(line.0);
        }
    }

    impl slog::Serializer for LogLine {
        fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments<'_>) -> slog::Result {
            write!(self.0, " {key}={val}").map_err(slog::Error::Fmt)
        }
    }
}
