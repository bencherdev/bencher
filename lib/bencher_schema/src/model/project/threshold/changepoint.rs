use bencher_changepoint::Changepoint;
use bencher_json::{
    ChangepointUuid, DateTime, JsonChangepoint, MetricName,
    project::changepoint::{
        ChangepointCount, ChangepointStatus, JsonChangepointScan, UpdateChangepointStatus,
    },
};
use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _, SqliteExpressionMethods as _,
};
use dropshot::HttpError;

use super::{QueryThreshold, ThresholdId, model::ModelId};
use crate::{
    context::DbConnection,
    error::{BencherResource, assert_parentage, resource_not_found_err, resource_not_found_error},
    macros::{fn_get::fn_get, sql::last_insert_rowid},
    model::project::{
        ProjectId, QueryProject,
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::{head::HeadId, version::VersionId},
        measure::MeasureId,
        testbed::TestbedId,
        variant::{QueryVariant, VariantId},
    },
    model::spec::SpecId,
    schema::{self, changepoint as changepoint_table, changepoint_scan as changepoint_scan_table},
};

crate::macros::typed_id::typed_id!(ChangepointId);
crate::macros::typed_id::typed_id!(ChangepointScanId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = changepoint_table)]
pub struct QueryChangepoint {
    pub id: ChangepointId,
    pub uuid: ChangepointUuid,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub benchmark_id: BenchmarkId,
    pub variant_id: VariantId,
    pub measure_id: MeasureId,
    pub metric: MetricName,
    pub version_id: VersionId,
    pub threshold_id: ThresholdId,
    pub model_id: ModelId,
    pub mean_before: f64,
    pub mean_after: f64,
    pub stddev_before: f64,
    pub stddev_after: f64,
    pub pvalue: f64,
    pub tstatistic: Option<f64>,
    pub degrees_of_freedom: ChangepointCount,
    pub status: ChangepointState,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryChangepoint {
    fn_get!(changepoint, ChangepointId);

    /// The changepoint of the project with this UUID, which is never a candidate.
    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: ChangepointUuid,
    ) -> Result<Self, HttpError> {
        schema::changepoint::table
            .filter(schema::changepoint::uuid.eq(uuid))
            .filter(schema::changepoint::project_id.eq(project_id))
            .filter(schema::changepoint::status.ne(ChangepointState::Candidate))
            .select(Self::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(Changepoint, (project_id, uuid)))
    }

    /// Every changepoint on the line, whatever its status, oldest first.
    pub fn on_line(
        conn: &mut DbConnection,
        line: &ChangepointLine,
    ) -> diesel::QueryResult<Vec<Self>> {
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
        schema::changepoint::table
            .filter(schema::changepoint::head_id.eq(head_id))
            .filter(schema::changepoint::testbed_id.eq(testbed_id))
            .filter(schema::changepoint::spec_id.is(spec_id))
            .filter(schema::changepoint::benchmark_id.eq(benchmark_id))
            .filter(schema::changepoint::variant_id.eq(variant_id))
            .filter(schema::changepoint::measure_id.eq(measure_id))
            .filter(schema::changepoint::metric.eq(metric))
            .order(schema::changepoint::id)
            .select(Self::as_select())
            .load(conn)
    }

    /// Silence the active changepoints of a replaced head and delete its candidates,
    /// which no user saw. Every other changepoint keeps its status and `modified`.
    /// Returns the number of changepoints silenced.
    pub fn silence_all(
        conn: &mut DbConnection,
        head_id: HeadId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::delete(
            schema::changepoint::table
                .filter(schema::changepoint::head_id.eq(head_id))
                .filter(schema::changepoint::status.eq(ChangepointState::Candidate)),
        )
        .execute(conn)?;
        diesel::update(
            schema::changepoint::table
                .filter(schema::changepoint::head_id.eq(head_id))
                .filter(schema::changepoint::status.eq(ChangepointState::Active)),
        )
        .set(&UpdateChangepoint::state(ChangepointState::Silenced, now))
        .execute(conn)
    }

    /// The scans that found the changepoint, oldest first: every scan of a candidate, and every
    /// scan up to the confirming one of a confirmed changepoint.
    pub fn scans(
        conn: &mut DbConnection,
        id: ChangepointId,
    ) -> diesel::QueryResult<Vec<QueryChangepointScan>> {
        schema::changepoint_scan::table
            .filter(schema::changepoint_scan::changepoint_id.eq(id))
            .order((
                schema::changepoint_scan::created,
                schema::changepoint_scan::id,
            ))
            .select(QueryChangepointScan::as_select())
            .load(conn)
    }

    /// Count one more consecutive scan that found a candidate: record what it measured, which the
    /// candidate then holds as its own.
    /// Returns the number of changepoints counted, which is 0 for one that is not a candidate.
    pub fn count_scan(
        conn: &mut DbConnection,
        id: ChangepointId,
        changepoint: &Changepoint,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        Self::scan_candidate(conn, id, changepoint, ChangepointState::Candidate, now)
    }

    /// Make a candidate an active changepoint, which users see, with what its confirming scan
    /// measured. Its measured fields never change after.
    /// Returns the number of changepoints confirmed, which is 0 for one that is not a candidate.
    pub fn confirm(
        conn: &mut DbConnection,
        id: ChangepointId,
        changepoint: &Changepoint,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        Self::scan_candidate(conn, id, changepoint, ChangepointState::Active, now)
    }

    /// Record a scan of a candidate and give the candidate what it measured, leaving it in `state`.
    fn scan_candidate(
        conn: &mut DbConnection,
        id: ChangepointId,
        changepoint: &Changepoint,
        state: ChangepointState,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        let measured = Measured::of(changepoint)?;
        conn.transaction(|conn| {
            let scanned = diesel::update(
                schema::changepoint::table
                    .filter(schema::changepoint::id.eq(id))
                    .filter(schema::changepoint::status.eq(ChangepointState::Candidate)),
            )
            .set(&UpdateScannedChangepoint::new(measured, state, now))
            .execute(conn)?;
            if scanned > 0 {
                InsertChangepointScan::new(id, measured, now).insert(conn)?;
            }
            Ok(scanned)
        })
    }

    /// Delete the candidates of a threshold, which no user saw, once its changepoint slot is
    /// emptied or holds another model.
    /// Returns the number of candidates deleted.
    pub fn delete_candidates_of_threshold(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
    ) -> diesel::QueryResult<usize> {
        diesel::delete(
            schema::changepoint::table
                .filter(schema::changepoint::threshold_id.eq(threshold_id))
                .filter(schema::changepoint::status.eq(ChangepointState::Candidate)),
        )
        .execute(conn)
    }

    /// Withdraw an active changepoint whose step moved or vanished.
    /// Returns the number of changepoints withdrawn, which is 0 for one that is not active:
    /// a dismissed or silenced changepoint is settled and is never rewritten.
    pub fn withdraw(
        conn: &mut DbConnection,
        id: ChangepointId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        Self::move_state(
            conn,
            id,
            ChangepointState::Active,
            ChangepointState::Withdrawn,
            now,
        )
    }

    /// Delete a candidate that a scan did not find again.
    /// Returns the number of changepoints deleted, which is 0 for one that is not a candidate.
    pub fn delete_candidate(
        conn: &mut DbConnection,
        id: ChangepointId,
    ) -> diesel::QueryResult<usize> {
        diesel::delete(
            schema::changepoint::table
                .filter(schema::changepoint::id.eq(id))
                .filter(schema::changepoint::status.eq(ChangepointState::Candidate)),
        )
        .execute(conn)
    }

    /// Change a user's view of a changepoint, between active and dismissed.
    /// Returns the number of changepoints changed, which is 0 for one a user may not change.
    pub fn set_status(
        conn: &mut DbConnection,
        id: ChangepointId,
        status: UpdateChangepointStatus,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::changepoint::table
                .filter(schema::changepoint::id.eq(id))
                .filter(
                    schema::changepoint::status
                        .eq_any([ChangepointState::Active, ChangepointState::Dismissed]),
                ),
        )
        .set(&UpdateChangepoint::state(
            ChangepointStatus::from(status).into(),
            now,
        ))
        .execute(conn)
    }

    fn move_state(
        conn: &mut DbConnection,
        id: ChangepointId,
        from: ChangepointState,
        to: ChangepointState,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::changepoint::table
                .filter(schema::changepoint::id.eq(id))
                .filter(schema::changepoint::status.eq(from)),
        )
        .set(&UpdateChangepoint::state(to, now))
        .execute(conn)
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonChangepoint, HttpError> {
        let Self {
            uuid,
            project_id,
            head_id,
            spec_id,
            benchmark_id,
            variant_id,
            metric,
            version_id,
            threshold_id,
            model_id,
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            status: state,
            created,
            modified,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Changepoint,
            project_id,
        );
        let Some(status) = state.status() else {
            return Err(resource_not_found_error(
                BencherResource::Changepoint,
                uuid,
                diesel::result::Error::NotFound,
            ));
        };
        let threshold = QueryThreshold::get_alert_json(
            conn,
            threshold_id,
            model_id,
            head_id,
            version_id,
            spec_id,
        )?;
        let query_benchmark = QueryBenchmark::get(conn, benchmark_id)?;
        let variant =
            QueryVariant::get(conn, variant_id)?.into_json_for_benchmark(&query_benchmark);
        let percent_change = percent_change(mean_before, mean_after);
        Ok(JsonChangepoint {
            uuid,
            threshold,
            benchmark: query_benchmark.into_json_for_project(project),
            variant,
            metric,
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom: degrees_of_freedom.into(),
            percent_change: percent_change.is_finite().then_some(percent_change),
            scans: None,
            status,
            created,
            modified,
        })
    }

    /// The changepoint as its own view serves it, with its scans.
    pub fn into_json_with_scans_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonChangepoint, HttpError> {
        let id = self.id;
        let mut json = self.into_json_for_project(conn, project)?;
        let scans = Self::scans(conn, id)
            .map_err(resource_not_found_err!(Changepoint, json.uuid))?
            .into_iter()
            .map(QueryChangepointScan::into_json)
            .collect();
        json.scans = Some(scans);
        Ok(json)
    }
}

/// The change from the mean before to the mean after, in percent, as the port's
/// `forward_change_percent` computes it: 0 when the mean before is 0.
fn percent_change(mean_before: f64, mean_after: f64) -> f64 {
    if mean_before == 0.0 {
        0.0
    } else {
        (mean_after / mean_before - 1.0) * 100.0
    }
}

/// The line a changepoint is on: one series of one threshold on one head, read under one spec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChangepointLine {
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub benchmark_id: BenchmarkId,
    pub variant_id: VariantId,
    pub measure_id: MeasureId,
    pub metric: MetricName,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = changepoint_table)]
pub struct InsertChangepoint {
    pub uuid: ChangepointUuid,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub benchmark_id: BenchmarkId,
    pub variant_id: VariantId,
    pub measure_id: MeasureId,
    pub metric: MetricName,
    pub version_id: VersionId,
    pub threshold_id: ThresholdId,
    pub model_id: ModelId,
    pub mean_before: f64,
    pub mean_after: f64,
    pub stddev_before: f64,
    pub stddev_after: f64,
    pub pvalue: f64,
    pub tstatistic: Option<f64>,
    pub degrees_of_freedom: ChangepointCount,
    pub status: ChangepointState,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertChangepoint {
    /// Insert a candidate found by one scan of the line, at the version it lands on,
    /// judged against the threshold's changepoint model, with that scan. A t statistic that is
    /// not finite, as between 2 constant sides, is stored as none.
    pub fn candidate(
        conn: &mut DbConnection,
        line: ChangepointLine,
        version_id: VersionId,
        threshold_id: ThresholdId,
        model_id: ModelId,
        changepoint: &Changepoint,
        now: DateTime,
    ) -> diesel::QueryResult<ChangepointId> {
        Self::insert(
            conn,
            line,
            version_id,
            (threshold_id, model_id),
            changepoint,
            ChangepointState::Candidate,
            now,
        )
    }

    /// Insert an active changepoint that its first scan confirms, under a changepoint model that
    /// takes 1 confirming scan, with that scan.
    pub fn confirmed(
        conn: &mut DbConnection,
        line: ChangepointLine,
        version_id: VersionId,
        threshold_id: ThresholdId,
        model_id: ModelId,
        changepoint: &Changepoint,
        now: DateTime,
    ) -> diesel::QueryResult<ChangepointId> {
        Self::insert(
            conn,
            line,
            version_id,
            (threshold_id, model_id),
            changepoint,
            ChangepointState::Active,
            now,
        )
    }

    fn insert(
        conn: &mut DbConnection,
        line: ChangepointLine,
        version_id: VersionId,
        (threshold_id, model_id): (ThresholdId, ModelId),
        changepoint: &Changepoint,
        status: ChangepointState,
        now: DateTime,
    ) -> diesel::QueryResult<ChangepointId> {
        let ChangepointLine {
            project_id,
            head_id,
            testbed_id,
            spec_id,
            benchmark_id,
            variant_id,
            measure_id,
            metric,
        } = line;
        let measured = Measured::of(changepoint)?;
        let Measured {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
        } = measured;
        let insert_changepoint = Self {
            uuid: ChangepointUuid::new(),
            project_id,
            head_id,
            testbed_id,
            spec_id,
            benchmark_id,
            variant_id,
            measure_id,
            metric,
            version_id,
            threshold_id,
            model_id,
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            status,
            created: now,
            modified: now,
        };
        conn.transaction(|conn| {
            diesel::insert_into(schema::changepoint::table)
                .values(&insert_changepoint)
                .execute(conn)?;
            let id = diesel::select(last_insert_rowid()).get_result(conn)?;
            InsertChangepointScan::new(id, measured, now).insert(conn)?;
            Ok(id)
        })
    }
}

/// What one scan measured at a changepoint, as it is stored.
#[derive(Debug, Clone, Copy)]
struct Measured {
    mean_before: f64,
    mean_after: f64,
    stddev_before: f64,
    stddev_after: f64,
    pvalue: f64,
    /// None when it is not finite, as it is between 2 constant sides.
    tstatistic: Option<f64>,
    degrees_of_freedom: ChangepointCount,
}

impl Measured {
    fn of(changepoint: &Changepoint) -> diesel::QueryResult<Self> {
        let Changepoint {
            index: _,
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
        } = *changepoint;
        let degrees_of_freedom = u32::try_from(degrees_of_freedom)
            .map(ChangepointCount)
            .map_err(|error| diesel::result::Error::SerializationError(Box::new(error)))?;
        Ok(Self {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic: tstatistic.is_finite().then_some(tstatistic),
            degrees_of_freedom,
        })
    }
}

/// A candidate as a scan that finds it again leaves it: with what that scan measured, and still a
/// candidate or now confirmed.
#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = changepoint_table, treat_none_as_null = true)]
struct UpdateScannedChangepoint {
    mean_before: f64,
    mean_after: f64,
    stddev_before: f64,
    stddev_after: f64,
    pvalue: f64,
    tstatistic: Option<f64>,
    degrees_of_freedom: ChangepointCount,
    status: ChangepointState,
    modified: DateTime,
}

impl UpdateScannedChangepoint {
    fn new(measured: Measured, status: ChangepointState, now: DateTime) -> Self {
        let Measured {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
        } = measured;
        Self {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            status,
            modified: now,
        }
    }
}

/// One scan that found a changepoint, up to its confirmation.
#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = changepoint_scan_table)]
pub struct QueryChangepointScan {
    pub id: ChangepointScanId,
    pub changepoint_id: ChangepointId,
    pub mean_before: f64,
    pub mean_after: f64,
    pub stddev_before: f64,
    pub stddev_after: f64,
    pub pvalue: f64,
    pub tstatistic: Option<f64>,
    pub degrees_of_freedom: ChangepointCount,
    pub created: DateTime,
}

impl QueryChangepointScan {
    pub fn into_json(self) -> JsonChangepointScan {
        let Self {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            created,
            ..
        } = self;
        let percent_change = percent_change(mean_before, mean_after);
        JsonChangepointScan {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom: degrees_of_freedom.into(),
            percent_change: percent_change.is_finite().then_some(percent_change),
            created,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = changepoint_scan_table)]
struct InsertChangepointScan {
    changepoint_id: ChangepointId,
    mean_before: f64,
    mean_after: f64,
    stddev_before: f64,
    stddev_after: f64,
    pvalue: f64,
    tstatistic: Option<f64>,
    degrees_of_freedom: ChangepointCount,
    created: DateTime,
}

impl InsertChangepointScan {
    fn new(changepoint_id: ChangepointId, measured: Measured, now: DateTime) -> Self {
        let Measured {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
        } = measured;
        Self {
            changepoint_id,
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            created: now,
        }
    }

    fn insert(&self, conn: &mut DbConnection) -> diesel::QueryResult<usize> {
        diesel::insert_into(schema::changepoint_scan::table)
            .values(self)
            .execute(conn)
    }
}

/// The only change a changepoint takes once it is confirmed: its status. Its measured fields
/// never change after.
#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = changepoint_table)]
struct UpdateChangepoint {
    status: ChangepointState,
    modified: DateTime,
}

impl UpdateChangepoint {
    fn state(status: ChangepointState, now: DateTime) -> Self {
        Self {
            status,
            modified: now,
        }
    }
}

/// What a changepoint's status column holds: a candidate, which no user sees,
/// or a status a user sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub enum ChangepointState {
    Candidate,
    Active,
    Dismissed,
    Silenced,
    Withdrawn,
}

const CANDIDATE_INT: i32 = -1;
const ACTIVE_INT: i32 = 0;
const DISMISSED_INT: i32 = 1;
const SILENCED_INT: i32 = 10;
const WITHDRAWN_INT: i32 = 11;

impl ChangepointState {
    /// The status a user sees, or none for a candidate.
    pub fn status(self) -> Option<ChangepointStatus> {
        match self {
            Self::Candidate => None,
            Self::Active => Some(ChangepointStatus::Active),
            Self::Dismissed => Some(ChangepointStatus::Dismissed),
            Self::Silenced => Some(ChangepointStatus::Silenced),
            Self::Withdrawn => Some(ChangepointStatus::Withdrawn),
        }
    }
}

impl From<ChangepointStatus> for ChangepointState {
    fn from(status: ChangepointStatus) -> Self {
        match status {
            ChangepointStatus::Active => Self::Active,
            ChangepointStatus::Dismissed => Self::Dismissed,
            ChangepointStatus::Silenced => Self::Silenced,
            ChangepointStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChangepointStateError {
    #[error("Invalid changepoint status value: {0}")]
    Invalid(i32),
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for ChangepointState
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Candidate => CANDIDATE_INT.to_sql(out),
            Self::Active => ACTIVE_INT.to_sql(out),
            Self::Dismissed => DISMISSED_INT.to_sql(out),
            Self::Silenced => SILENCED_INT.to_sql(out),
            Self::Withdrawn => WITHDRAWN_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for ChangepointState
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            CANDIDATE_INT => Ok(Self::Candidate),
            ACTIVE_INT => Ok(Self::Active),
            DISMISSED_INT => Ok(Self::Dismissed),
            SILENCED_INT => Ok(Self::Silenced),
            WITHDRAWN_INT => Ok(Self::Withdrawn),
            value => Err(Box::new(ChangepointStateError::Invalid(value))),
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_changepoint::Changepoint;
    use bencher_json::{
        DateTime,
        project::changepoint::{ChangepointCount, UpdateChangepointStatus},
    };

    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{
        ChangepointLine, ChangepointState, InsertChangepoint, QueryChangepoint,
        QueryChangepointScan, percent_change,
    };
    use crate::{
        model::project::threshold::model::ModelId,
        schema,
        test_util::{
            ChangepointFixture, CreateSpecArgs, MEASURED_STEP, create_base_entities,
            create_branch_with_head, create_changepoint, create_changepoint_fixture, create_spec,
            find_changepoint, setup_test_db,
        },
    };

    const LATER: DateTime = DateTime::TEST;

    fn fixture(conn: &mut diesel::SqliteConnection) -> ChangepointFixture {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        create_changepoint_fixture(conn, base.project_id, branch, 1)
    }

    fn earlier() -> DateTime {
        DateTime::try_from(LATER.timestamp() - 1_000).expect("a time")
    }

    /// The means, the standard deviations, the p value, the t statistic, and the degrees of freedom.
    type Measured = (f64, f64, f64, f64, f64, Option<f64>, ChangepointCount);

    /// The measured fields of a changepoint, as the port gave them.
    fn measured(changepoint: &QueryChangepoint) -> Measured {
        (
            changepoint.mean_before,
            changepoint.mean_after,
            changepoint.stddev_before,
            changepoint.stddev_after,
            changepoint.pvalue,
            changepoint.tstatistic,
            changepoint.degrees_of_freedom,
        )
    }

    fn step_measured() -> Measured {
        measured_by(&MEASURED_STEP)
    }

    /// The measured fields the port's changepoint is stored as.
    fn measured_by(changepoint: &Changepoint) -> Measured {
        let Changepoint {
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic,
            degrees_of_freedom,
            ..
        } = *changepoint;
        (
            mean_before,
            mean_after,
            stddev_before,
            stddev_after,
            pvalue,
            tstatistic.is_finite().then_some(tstatistic),
            ChangepointCount(u32::try_from(degrees_of_freedom).expect("a count")),
        )
    }

    /// What each scan of a changepoint measured, oldest first.
    fn scans_measured(
        conn: &mut diesel::SqliteConnection,
        id: super::ChangepointId,
    ) -> Vec<Measured> {
        QueryChangepoint::scans(conn, id)
            .expect("Failed to read the scans")
            .into_iter()
            .map(
                |QueryChangepointScan {
                     mean_before,
                     mean_after,
                     stddev_before,
                     stddev_after,
                     pvalue,
                     tstatistic,
                     degrees_of_freedom,
                     ..
                 }| {
                    (
                        mean_before,
                        mean_after,
                        stddev_before,
                        stddev_after,
                        pvalue,
                        tstatistic,
                        degrees_of_freedom,
                    )
                },
            )
            .collect()
    }

    /// A scan that measured the step as a little larger each time, its p value smaller.
    fn scan(number: u8) -> Changepoint {
        Changepoint {
            mean_after: 110.0 + f64::from(number),
            pvalue: 0.000_1 / f64::from(number),
            tstatistic: -21.5 - f64::from(number),
            degrees_of_freedom: 98 + usize::from(number),
            ..MEASURED_STEP
        }
    }

    fn scan_count(conn: &mut diesel::SqliteConnection, id: super::ChangepointId) -> usize {
        schema::changepoint_scan::table
            .filter(schema::changepoint_scan::changepoint_id.eq(id))
            .count()
            .get_result::<i64>(conn)
            .expect("Failed to count the scans")
            .try_into()
            .expect("a count")
    }

    #[test]
    fn a_candidate_holds_what_one_scan_measured() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);

        let id = InsertChangepoint::candidate(
            &mut conn,
            fixture.line.clone(),
            fixture.version_id,
            fixture.threshold_id,
            fixture.model_id,
            &MEASURED_STEP,
            LATER,
        )
        .expect("Failed to insert a candidate");

        let candidate = find_changepoint(&mut conn, id).expect("a candidate");
        assert_eq!(candidate.status, ChangepointState::Candidate);
        assert_eq!(scans_measured(&mut conn, id), [step_measured()]);
        assert_eq!(measured(&candidate), step_measured());
        assert_eq!(candidate.head_id, fixture.line.head_id);
        assert_eq!(candidate.variant_id, fixture.line.variant_id);
        assert_eq!(candidate.metric, fixture.line.metric);
        assert_eq!(candidate.version_id, fixture.version_id);
        assert_eq!(candidate.model_id, fixture.model_id);
        assert_eq!((candidate.created, candidate.modified), (LATER, LATER));
    }

    // Kills a t statistic stored as the port gave it when it is infinite, as it is between 2
    // constant sides.
    #[test]
    fn a_candidate_stores_an_infinite_t_statistic_as_none() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let constant_sides = Changepoint {
            stddev_before: 0.0,
            stddev_after: 0.0,
            pvalue: 0.0,
            tstatistic: f64::NEG_INFINITY,
            ..MEASURED_STEP
        };

        let id = InsertChangepoint::candidate(
            &mut conn,
            fixture.line.clone(),
            fixture.version_id,
            fixture.threshold_id,
            fixture.model_id,
            &constant_sides,
            LATER,
        )
        .expect("Failed to insert a candidate");

        let candidate = find_changepoint(&mut conn, id).expect("a candidate");
        assert_eq!(candidate.tstatistic, None);
        assert_eq!(
            (
                candidate.mean_before,
                candidate.mean_after,
                candidate.pvalue
            ),
            (100.0, 110.0, 0.0)
        );
    }

    // Kills a line read that leaves out any status, or that gives its rows newest first.
    #[test]
    fn a_line_holds_its_changepoints_in_every_status_oldest_first() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let ids = [
            ChangepointState::Withdrawn,
            ChangepointState::Candidate,
            ChangepointState::Active,
            ChangepointState::Silenced,
            ChangepointState::Dismissed,
        ]
        .map(|state| create_changepoint(&mut conn, &fixture, state, earlier()));

        let found = QueryChangepoint::on_line(&mut conn, &fixture.line)
            .expect("Failed to read the line")
            .into_iter()
            .map(|changepoint| changepoint.id)
            .collect::<Vec<_>>();
        assert_eq!(found, ids);
    }

    #[test]
    fn a_line_under_another_spec_is_another_line() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let [spec_a, spec_b] = [("a", 4), ("b", 8)].map(|(label, cpu)| {
            create_spec(
                &mut conn,
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
        });
        let under = |spec_id| ChangepointFixture {
            line: ChangepointLine {
                spec_id,
                ..fixture.line.clone()
            },
            ..fixture.clone()
        };
        let lines = [Some(spec_a), Some(spec_b), None].map(under);
        let ids = lines
            .clone()
            .map(|line| create_changepoint(&mut conn, &line, ChangepointState::Active, earlier()));

        for (line, id) in lines.iter().zip(ids) {
            let found = QueryChangepoint::on_line(&mut conn, &line.line)
                .expect("Failed to read the line")
                .into_iter()
                .map(|changepoint| changepoint.id)
                .collect::<Vec<_>>();
            assert_eq!(found, [id], "{:?}", line.line.spec_id);
        }
    }

    #[test]
    fn a_confirming_scan_counts_only_for_a_candidate() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let candidate =
            create_changepoint(&mut conn, &fixture, ChangepointState::Candidate, earlier());
        let active = create_changepoint(&mut conn, &fixture, ChangepointState::Active, earlier());

        for _ in 0..2 {
            assert_eq!(
                QueryChangepoint::count_scan(&mut conn, candidate, &MEASURED_STEP, LATER),
                Ok(1)
            );
        }
        assert_eq!(
            QueryChangepoint::count_scan(&mut conn, active, &MEASURED_STEP, LATER),
            Ok(0)
        );

        assert_eq!(scan_count(&mut conn, candidate), 3);
        let candidate = find_changepoint(&mut conn, candidate).expect("a candidate");
        assert_eq!(candidate.modified, LATER);
        assert_eq!(scan_count(&mut conn, active), 1);
        let active = find_changepoint(&mut conn, active).expect("an active changepoint");
        assert_eq!(active.modified, earlier());
    }

    // Kills a scan that is not recorded, recorded out of order, or left out of the candidate's own
    // fields, and a t statistic that a later scan measures as infinite kept from an earlier scan.
    #[test]
    fn three_scans_give_three_scan_rows_and_the_candidate_holds_the_third() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let id = InsertChangepoint::candidate(
            &mut conn,
            fixture.line.clone(),
            fixture.version_id,
            fixture.threshold_id,
            fixture.model_id,
            &scan(1),
            earlier(),
        )
        .expect("Failed to insert a candidate");
        let constant_sides = Changepoint {
            stddev_before: 0.0,
            stddev_after: 0.0,
            pvalue: 0.0,
            tstatistic: f64::NEG_INFINITY,
            ..scan(3)
        };
        for changepoint in [scan(2), constant_sides] {
            assert_eq!(
                QueryChangepoint::count_scan(&mut conn, id, &changepoint, LATER),
                Ok(1)
            );
        }

        assert_eq!(
            scans_measured(&mut conn, id),
            [scan(1), scan(2), constant_sides].map(|scan| measured_by(&scan))
        );
        let candidate = find_changepoint(&mut conn, id).expect("a candidate");
        assert_eq!(candidate.status, ChangepointState::Candidate);
        assert_eq!(measured(&candidate), measured_by(&constant_sides));
        assert_eq!(candidate.tstatistic, None);
        assert_eq!(candidate.modified, LATER);
    }

    // Kills a confirmation that keeps an earlier scan's fields or records no scan, and a scan after
    // the confirmation that writes the changepoint's fields or a scan row.
    #[test]
    fn a_confirmation_freezes_what_the_confirming_scan_measured() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let id = InsertChangepoint::candidate(
            &mut conn,
            fixture.line.clone(),
            fixture.version_id,
            fixture.threshold_id,
            fixture.model_id,
            &scan(1),
            earlier(),
        )
        .expect("Failed to insert a candidate");

        assert_eq!(
            QueryChangepoint::confirm(&mut conn, id, &scan(2), LATER),
            Ok(1)
        );
        let confirmed = find_changepoint(&mut conn, id).expect("an active changepoint");
        assert_eq!(confirmed.status, ChangepointState::Active);
        assert_eq!(measured(&confirmed), measured_by(&scan(2)));

        assert_eq!(
            QueryChangepoint::count_scan(&mut conn, id, &scan(3), LATER),
            Ok(0)
        );
        assert_eq!(
            QueryChangepoint::confirm(&mut conn, id, &scan(3), LATER),
            Ok(0)
        );
        let kept = find_changepoint(&mut conn, id).expect("an active changepoint");
        assert_eq!(measured(&kept), measured_by(&scan(2)));
        assert_eq!(
            scans_measured(&mut conn, id),
            [scan(1), scan(2)].map(|scan| measured_by(&scan))
        );
    }

    // Kills a changepoint confirmed at its first scan written as a candidate or with no scan.
    #[test]
    fn a_changepoint_its_first_scan_confirms_is_active_with_that_scan() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let id = InsertChangepoint::confirmed(
            &mut conn,
            fixture.line.clone(),
            fixture.version_id,
            fixture.threshold_id,
            fixture.model_id,
            &MEASURED_STEP,
            LATER,
        )
        .expect("Failed to insert a confirmed changepoint");

        let active = find_changepoint(&mut conn, id).expect("an active changepoint");
        assert_eq!(active.status, ChangepointState::Active);
        assert_eq!(measured(&active), step_measured());
        assert_eq!(scans_measured(&mut conn, id), [step_measured()]);
    }

    #[test]
    fn confirming_a_candidate_makes_it_active_with_what_it_measured() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let candidate =
            create_changepoint(&mut conn, &fixture, ChangepointState::Candidate, earlier());

        assert_eq!(
            QueryChangepoint::confirm(&mut conn, candidate, &MEASURED_STEP, LATER),
            Ok(1)
        );
        assert_eq!(
            QueryChangepoint::confirm(&mut conn, candidate, &MEASURED_STEP, LATER),
            Ok(0),
            "an active changepoint is not a candidate"
        );

        let active = find_changepoint(&mut conn, candidate).expect("an active changepoint");
        assert_eq!(active.status, ChangepointState::Active);
        assert_eq!(active.modified, LATER);
        assert_eq!(measured(&active), step_measured());
    }

    #[test]
    fn withdrawing_takes_back_only_an_active_changepoint() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let active = create_changepoint(&mut conn, &fixture, ChangepointState::Active, earlier());

        assert_eq!(QueryChangepoint::withdraw(&mut conn, active, LATER), Ok(1));
        let withdrawn = find_changepoint(&mut conn, active).expect("a withdrawn changepoint");
        assert_eq!(withdrawn.status, ChangepointState::Withdrawn);
        assert_eq!(withdrawn.modified, LATER);

        for state in [
            ChangepointState::Candidate,
            ChangepointState::Dismissed,
            ChangepointState::Silenced,
            ChangepointState::Withdrawn,
        ] {
            let id = create_changepoint(&mut conn, &fixture, state, earlier());
            assert_eq!(
                QueryChangepoint::withdraw(&mut conn, id, LATER),
                Ok(0),
                "{state:?}"
            );
            let kept = find_changepoint(&mut conn, id).expect("a kept changepoint");
            assert_eq!(
                (kept.status, kept.modified),
                (state, earlier()),
                "{state:?}"
            );
        }
    }

    // Kills a deleted candidate that leaves its scans behind.
    #[test]
    fn deleting_a_candidate_deletes_only_a_candidate() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let candidate =
            create_changepoint(&mut conn, &fixture, ChangepointState::Candidate, earlier());

        assert_eq!(
            QueryChangepoint::count_scan(&mut conn, candidate, &MEASURED_STEP, LATER),
            Ok(1)
        );
        assert_eq!(
            QueryChangepoint::delete_candidate(&mut conn, candidate),
            Ok(1)
        );
        assert!(find_changepoint(&mut conn, candidate).is_none());
        assert_eq!(scan_count(&mut conn, candidate), 0, "its scans go with it");

        for state in [
            ChangepointState::Active,
            ChangepointState::Dismissed,
            ChangepointState::Silenced,
            ChangepointState::Withdrawn,
        ] {
            let id = create_changepoint(&mut conn, &fixture, state, earlier());
            assert_eq!(
                QueryChangepoint::delete_candidate(&mut conn, id),
                Ok(0),
                "{state:?}"
            );
            assert!(find_changepoint(&mut conn, id).is_some(), "{state:?}");
        }
    }

    #[test]
    fn a_user_moves_a_changepoint_only_between_active_and_dismissed() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let id = create_changepoint(&mut conn, &fixture, ChangepointState::Active, earlier());

        assert_eq!(
            QueryChangepoint::set_status(&mut conn, id, UpdateChangepointStatus::Dismissed, LATER),
            Ok(1)
        );
        assert_eq!(
            find_changepoint(&mut conn, id).map(|c| c.status),
            Some(ChangepointState::Dismissed)
        );
        assert_eq!(
            QueryChangepoint::set_status(&mut conn, id, UpdateChangepointStatus::Active, LATER),
            Ok(1)
        );
        assert_eq!(
            find_changepoint(&mut conn, id).map(|c| c.status),
            Some(ChangepointState::Active)
        );

        for state in [
            ChangepointState::Candidate,
            ChangepointState::Silenced,
            ChangepointState::Withdrawn,
        ] {
            let id = create_changepoint(&mut conn, &fixture, state, earlier());
            assert_eq!(
                QueryChangepoint::set_status(
                    &mut conn,
                    id,
                    UpdateChangepointStatus::Dismissed,
                    LATER
                ),
                Ok(0),
                "{state:?}"
            );
            let kept = find_changepoint(&mut conn, id).expect("a kept changepoint");
            assert_eq!(
                (kept.status, kept.modified),
                (state, earlier()),
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_model_a_changepoint_was_judged_against_is_kept() {
        let mut conn = setup_test_db();
        let fixture = fixture(&mut conn);
        let id = create_changepoint(&mut conn, &fixture, ChangepointState::Active, earlier());
        // A model replaced in its slot is no longer the threshold's, and only the changepoint
        // still points at it.
        diesel::update(
            schema::threshold::table.filter(schema::threshold::id.eq(fixture.threshold_id)),
        )
        .set(schema::threshold::changepoint_model_id.eq(None::<ModelId>))
        .execute(&mut conn)
        .expect("Failed to empty the changepoint slot");
        let delete_model = |conn: &mut diesel::SqliteConnection| {
            diesel::delete(schema::model::table.filter(schema::model::id.eq(fixture.model_id)))
                .execute(conn)
        };

        let refused = delete_model(&mut conn).expect_err("the changepoint holds its model");
        assert!(
            matches!(
                refused,
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                    _
                )
            ),
            "{refused:?}"
        );
        assert!(find_changepoint(&mut conn, id).is_some());

        diesel::delete(schema::changepoint::table.filter(schema::changepoint::id.eq(id)))
            .execute(&mut conn)
            .expect("Failed to delete the changepoint");
        assert_eq!(delete_model(&mut conn), Ok(1));
    }

    #[test]
    fn the_percent_change_is_the_ports_forward_change() {
        for (mean_before, mean_after) in [
            (100.0, 110.0),
            (110.0, 100.0),
            (-4.0, 2.0),
            (0.0, 5.0),
            (0.0, 0.0),
            (3.0, 0.0),
        ] {
            let port = Changepoint {
                mean_before,
                mean_after,
                ..MEASURED_STEP
            };
            assert_eq!(
                percent_change(mean_before, mean_after).to_bits(),
                port.forward_change_percent().to_bits(),
                "{mean_before} to {mean_after}"
            );
        }
        assert_eq!(
            percent_change(0.0, 5.0).to_bits(),
            0.0f64.to_bits(),
            "a mean before of 0 gives 0"
        );
    }
}
