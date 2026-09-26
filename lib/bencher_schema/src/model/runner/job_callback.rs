use bencher_callback::SealedRequest;
use bencher_json::{
    DateTime,
    runner::{CALLBACK_JOB_STATUSES, JobCallbackState, JsonJobCallback},
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
    backend::Backend,
    deserialize::{self, FromSql},
    result::QueryResult,
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Integer,
    sqlite::Sqlite,
};
use http::StatusCode;

use super::JobId;
use crate::{
    context::DbConnection,
    schema::{self, job_callback as job_callback_table},
};

const PENDING_INT: i32 = 0;
const DELIVERING_INT: i32 = 1;
const DELIVERED_INT: i32 = 2;
const FAILED_INT: i32 = 3;
const SKIPPED_INT: i32 = 4;

#[derive(Debug, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_callback_table)]
pub struct QueryJobCallback {
    pub job_id: JobId,
    pub request: Option<SealedRequest>,
    pub state: CallbackState,
    pub attempts: i32,
    pub status: Option<HttpStatus>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryJobCallback {
    pub fn get(conn: &mut DbConnection, job_id: JobId) -> QueryResult<Self> {
        schema::job_callback::table
            .filter(schema::job_callback::job_id.eq(job_id))
            .select(Self::as_select())
            .first(conn)
    }

    /// Move a pending callback to delivering. Of any number of concurrent claims, exactly one wins.
    pub fn claim(conn: &mut DbConnection, job_id: JobId, now: DateTime) -> QueryResult<bool> {
        transition(
            conn,
            job_id,
            CallbackState::Pending,
            CallbackState::Delivering,
            now,
        )
    }

    /// Count an attempt, and keep its status if the receiver answered.
    pub fn record_attempt(
        conn: &mut DbConnection,
        job_id: JobId,
        status: Option<StatusCode>,
        now: DateTime,
    ) -> QueryResult<bool> {
        let delivering = schema::job_callback::table
            .filter(schema::job_callback::job_id.eq(job_id))
            .filter(schema::job_callback::state.eq(CallbackState::Delivering));
        let attempt = (
            schema::job_callback::attempts.eq(schema::job_callback::attempts + 1),
            schema::job_callback::modified.eq(now),
        );
        if let Some(status) = status {
            diesel::update(delivering)
                .set((
                    attempt,
                    schema::job_callback::status.eq(HttpStatus::from(status)),
                ))
                .execute(conn)
        } else {
            diesel::update(delivering).set(attempt).execute(conn)
        }
        .map(|updated| updated > 0)
    }

    /// Settle a delivering callback and drop its sealed request.
    pub fn finish(
        conn: &mut DbConnection,
        job_id: JobId,
        outcome: CallbackOutcome,
        now: DateTime,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::job_callback::table
                .filter(schema::job_callback::job_id.eq(job_id))
                .filter(schema::job_callback::state.eq(CallbackState::Delivering)),
        )
        .set((
            schema::job_callback::state.eq(CallbackState::from(outcome)),
            schema::job_callback::request.eq(None::<SealedRequest>),
            schema::job_callback::modified.eq(now),
        ))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// Return a delivering callback to pending, for a delivery cut short by shutdown.
    pub fn release(conn: &mut DbConnection, job_id: JobId, now: DateTime) -> QueryResult<bool> {
        transition(
            conn,
            job_id,
            CallbackState::Delivering,
            CallbackState::Pending,
            now,
        )
    }

    /// Return every delivering callback to pending, for the deliveries a crash cut short; it cannot tell those from a live claim, so it runs only before this process can claim.
    pub fn reset_delivering(conn: &mut DbConnection, now: DateTime) -> QueryResult<usize> {
        diesel::update(
            schema::job_callback::table
                .filter(schema::job_callback::state.eq(CallbackState::Delivering)),
        )
        .set((
            schema::job_callback::state.eq(CallbackState::Pending),
            schema::job_callback::modified.eq(now),
        ))
        .execute(conn)
    }

    /// The jobs whose pending callback is due, since each has reached a terminal status.
    pub fn pending_on_terminal_jobs(conn: &mut DbConnection) -> QueryResult<Vec<JobId>> {
        schema::job_callback::table
            .inner_join(schema::job::table)
            .filter(schema::job_callback::state.eq(CallbackState::Pending))
            .filter(schema::job::status.eq_any(CALLBACK_JOB_STATUSES))
            .order(schema::job_callback::job_id.asc())
            .select(schema::job_callback::job_id)
            .load(conn)
    }

    /// The callbacks that still hold a sealed request, counted by state so the partial index answers it.
    pub fn count_sealed(conn: &mut DbConnection) -> QueryResult<i64> {
        schema::job_callback::table
            .filter(
                schema::job_callback::state
                    .eq(CallbackState::Pending)
                    .or(schema::job_callback::state.eq(CallbackState::Delivering)),
            )
            .count()
            .get_result(conn)
    }
}

fn transition(
    conn: &mut DbConnection,
    job_id: JobId,
    from: CallbackState,
    to: CallbackState,
    now: DateTime,
) -> QueryResult<bool> {
    diesel::update(
        schema::job_callback::table
            .filter(schema::job_callback::job_id.eq(job_id))
            .filter(schema::job_callback::state.eq(from)),
    )
    .set((
        schema::job_callback::state.eq(to),
        schema::job_callback::modified.eq(now),
    ))
    .execute(conn)
    .map(|updated| updated > 0)
}

/// A callback's state and last status, without its sealed request, so a job list never loads one.
#[derive(Debug, Clone, Copy, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_callback_table)]
pub struct QueryJobCallbackView {
    pub state: CallbackState,
    pub status: Option<HttpStatus>,
}

impl From<QueryJobCallbackView> for JsonJobCallback {
    fn from(view: QueryJobCallbackView) -> Self {
        let QueryJobCallbackView { state, status } = view;
        Self {
            state: state.into(),
            status: status.map(u16::from),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_callback_table)]
pub struct InsertJobCallback {
    job_id: JobId,
    request: Option<SealedRequest>,
    state: CallbackState,
    created: DateTime,
    modified: DateTime,
}

impl InsertJobCallback {
    pub fn pending(job_id: JobId, request: SealedRequest, now: DateTime) -> Self {
        Self {
            job_id,
            request: Some(request),
            state: CallbackState::Pending,
            created: now,
            modified: now,
        }
    }

    /// A callback that is accepted but never sent, so nothing is sealed.
    pub fn skipped(job_id: JobId, now: DateTime) -> Self {
        Self {
            job_id,
            request: None,
            state: CallbackState::Skipped,
            created: now,
            modified: now,
        }
    }

    pub fn insert(&self, conn: &mut DbConnection) -> QueryResult<()> {
        diesel::insert_into(schema::job_callback::table)
            .values(self)
            .execute(conn)
            .map(|_| ())
    }
}

/// How a delivering callback ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackOutcome {
    Delivered,
    Failed,
}

impl From<CallbackOutcome> for CallbackState {
    fn from(outcome: CallbackOutcome) -> Self {
        match outcome {
            CallbackOutcome::Delivered => Self::Delivered,
            CallbackOutcome::Failed => Self::Failed,
        }
    }
}

/// The stored state of a callback: the public states, plus `Delivering` while a delivery holds its claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = Integer)]
#[repr(i32)]
pub enum CallbackState {
    Pending = PENDING_INT,
    Delivering = DELIVERING_INT,
    Delivered = DELIVERED_INT,
    Failed = FAILED_INT,
    Skipped = SKIPPED_INT,
}

impl From<CallbackState> for JobCallbackState {
    fn from(state: CallbackState) -> Self {
        match state {
            CallbackState::Pending | CallbackState::Delivering => Self::Pending,
            CallbackState::Delivered => Self::Delivered,
            CallbackState::Failed => Self::Failed,
            CallbackState::Skipped => Self::Skipped,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackStateError {
    #[error("Invalid callback state value: {0}")]
    Invalid(i32),
}

impl<DB> ToSql<Integer, DB> for CallbackState
where
    DB: Backend,
    i32: ToSql<Integer, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> serialize::Result {
        match self {
            Self::Pending => PENDING_INT.to_sql(out),
            Self::Delivering => DELIVERING_INT.to_sql(out),
            Self::Delivered => DELIVERED_INT.to_sql(out),
            Self::Failed => FAILED_INT.to_sql(out),
            Self::Skipped => SKIPPED_INT.to_sql(out),
        }
    }
}

impl<DB> FromSql<Integer, DB> for CallbackState
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            PENDING_INT => Ok(Self::Pending),
            DELIVERING_INT => Ok(Self::Delivering),
            DELIVERED_INT => Ok(Self::Delivered),
            FAILED_INT => Ok(Self::Failed),
            SKIPPED_INT => Ok(Self::Skipped),
            value => Err(Box::new(CallbackStateError::Invalid(value))),
        }
    }
}

/// The HTTP status of a callback's last response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = Integer)]
pub struct HttpStatus(StatusCode);

impl From<StatusCode> for HttpStatus {
    fn from(status: StatusCode) -> Self {
        Self(status)
    }
}

impl From<HttpStatus> for u16 {
    fn from(status: HttpStatus) -> Self {
        status.0.as_u16()
    }
}

impl ToSql<Integer, Sqlite> for HttpStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(i32::from(self.0.as_u16()));
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Integer, DB> for HttpStatus
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let status = u16::try_from(i32::from_sql(bytes)?)?;
        Ok(Self(StatusCode::from_u16(status)?))
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Barrier, thread};

    use bencher_callback::{CallbackKey, SealedRequest};
    use bencher_json::{DateTime, JobStatus, JobUuid, Secret};
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection,
        connection::SimpleConnection as _,
    };
    use http::StatusCode;
    use pretty_assertions::assert_eq;

    use super::{CallbackOutcome, CallbackState, HttpStatus, InsertJobCallback, QueryJobCallback};
    use crate::{
        context::DbConnection,
        model::runner::JobId,
        run_migrations, schema,
        test_util::{JobFixture, create_job, create_job_fixture, setup_test_db},
    };

    const MARKER: &[u8] = b"MARKER-5d0c9b1e";

    fn at(seconds: i64) -> DateTime {
        DateTime::from(DateTime::TEST.into_inner() + chrono::Duration::seconds(seconds))
    }

    fn sealed(job: JobUuid) -> SealedRequest {
        let secret: Secret = "job-callback-test-secret".parse().unwrap();
        let plaintext = [br#"{"marker":""#.as_slice(), MARKER, br#""}"#].concat();
        CallbackKey::new(&secret)
            .unwrap()
            .seal(job, &plaintext)
            .unwrap()
    }

    fn job_with(
        conn: &mut DbConnection,
        fixture: JobFixture,
        status: JobStatus,
        state: Option<CallbackState>,
    ) -> JobId {
        let job_id = create_job(conn, fixture, status);
        let Some(state) = state else {
            return job_id;
        };
        if state == CallbackState::Skipped {
            InsertJobCallback::skipped(job_id, DateTime::TEST)
                .insert(conn)
                .unwrap();
        } else {
            InsertJobCallback::pending(job_id, sealed(JobUuid::new()), DateTime::TEST)
                .insert(conn)
                .unwrap();
            set_state(conn, job_id, state);
        }
        job_id
    }

    fn callback(conn: &mut DbConnection, fixture: JobFixture, state: CallbackState) -> JobId {
        job_with(conn, fixture, JobStatus::Processed, Some(state))
    }

    fn set_state(conn: &mut DbConnection, job_id: JobId, state: CallbackState) {
        let updated = diesel::update(
            schema::job_callback::table.filter(schema::job_callback::job_id.eq(job_id)),
        )
        .set(schema::job_callback::state.eq(state))
        .execute(conn)
        .unwrap();
        assert_eq!(updated, 1, "the callback row exists");
    }

    fn get(conn: &mut DbConnection, job_id: JobId) -> QueryJobCallback {
        QueryJobCallback::get(conn, job_id).unwrap()
    }

    fn sealed_bytes(row: &QueryJobCallback) -> Option<Vec<u8>> {
        row.request
            .as_ref()
            .map(|request| request.as_ref().to_vec())
    }

    fn attempts_and_status(conn: &mut DbConnection, job_id: JobId) -> (i32, Option<HttpStatus>) {
        let row = get(conn, job_id);
        (row.attempts, row.status)
    }

    fn raw_state(conn: &mut DbConnection, job_id: JobId) -> i32 {
        schema::job_callback::table
            .filter(schema::job_callback::job_id.eq(job_id))
            .select(schema::job_callback::state)
            .first(conn)
            .unwrap()
    }

    // The migration's partial index names Pending and Delivering by their integers,
    // and a stored integer outlives any release, so the mapping never moves.
    #[test]
    fn state_integers_are_stable() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, CallbackState::Pending);
        for (state, integer) in [
            (CallbackState::Pending, 0),
            (CallbackState::Delivering, 1),
            (CallbackState::Delivered, 2),
            (CallbackState::Failed, 3),
            (CallbackState::Skipped, 4),
        ] {
            set_state(&mut conn, job_id, state);
            assert_eq!(raw_state(&mut conn, job_id), integer, "{state:?} is stored");
            diesel::update(
                schema::job_callback::table.filter(schema::job_callback::job_id.eq(job_id)),
            )
            .set(schema::job_callback::state.eq(integer))
            .execute(&mut conn)
            .unwrap();
            assert_eq!(get(&mut conn, job_id).state, state, "{integer} is loaded");
        }
    }

    #[test]
    fn a_stored_value_out_of_range_does_not_load() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let bad_state = callback(&mut conn, fixture, CallbackState::Pending);
        let bad_status = callback(&mut conn, fixture, CallbackState::Pending);
        diesel::update(
            schema::job_callback::table.filter(schema::job_callback::job_id.eq(bad_state)),
        )
        .set(schema::job_callback::state.eq(5))
        .execute(&mut conn)
        .unwrap();
        diesel::update(
            schema::job_callback::table.filter(schema::job_callback::job_id.eq(bad_status)),
        )
        .set(schema::job_callback::status.eq(Some(99)))
        .execute(&mut conn)
        .unwrap();

        QueryJobCallback::get(&mut conn, bad_state).unwrap_err();
        QueryJobCallback::get(&mut conn, bad_status).unwrap_err();
    }

    #[test]
    fn a_pending_row_holds_the_sealed_request() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = create_job(&mut conn, fixture, JobStatus::Pending);
        let job_uuid = JobUuid::new();
        let secret: Secret = "job-callback-test-secret".parse().unwrap();
        let key = CallbackKey::new(&secret).unwrap();
        let sealed = key.seal(job_uuid, MARKER).unwrap();
        let bytes = sealed.as_ref().to_vec();

        InsertJobCallback::pending(job_id, sealed, at(1))
            .insert(&mut conn)
            .unwrap();

        let row = get(&mut conn, job_id);
        assert_eq!(row.job_id, job_id);
        assert_eq!(row.state, CallbackState::Pending);
        assert_eq!(row.attempts, 0);
        assert_eq!(row.status, None);
        assert_eq!(row.created, at(1));
        assert_eq!(row.modified, at(1));
        let stored = row.request.expect("a pending row holds its sealed request");
        assert_eq!(stored.as_ref(), bytes.as_slice());
        assert_eq!(key.open(job_uuid, &stored).unwrap(), MARKER);
    }

    #[test]
    fn a_skipped_row_holds_nothing_sealed() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = create_job(&mut conn, fixture, JobStatus::Pending);

        InsertJobCallback::skipped(job_id, at(1))
            .insert(&mut conn)
            .unwrap();

        let row = get(&mut conn, job_id);
        assert_eq!(row.state, CallbackState::Skipped);
        assert!(row.request.is_none(), "a skipped row seals nothing");
        assert_eq!(row.attempts, 0);
        assert_eq!(row.status, None);
        assert_eq!(row.modified, at(1));
    }

    #[test]
    fn the_first_claim_wins_and_the_second_loses() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, CallbackState::Pending);

        assert!(
            QueryJobCallback::claim(&mut conn, job_id, at(1)).unwrap(),
            "first claim"
        );
        assert!(
            !QueryJobCallback::claim(&mut conn, job_id, at(2)).unwrap(),
            "second claim"
        );

        let row = get(&mut conn, job_id);
        assert_eq!(row.state, CallbackState::Delivering);
        assert_eq!(row.modified, at(1), "the losing claim writes nothing");
        assert!(row.request.is_some(), "a claim keeps the sealed request");
    }

    #[test]
    fn a_claim_on_any_other_state_or_no_row_loses() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for state in [
            CallbackState::Delivering,
            CallbackState::Delivered,
            CallbackState::Failed,
            CallbackState::Skipped,
        ] {
            let job_id = callback(&mut conn, fixture, state);
            assert!(
                !QueryJobCallback::claim(&mut conn, job_id, at(1)).unwrap(),
                "a claim on {state:?} loses"
            );
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state, "a losing claim leaves {state:?}");
            assert_eq!(
                row.modified,
                DateTime::TEST,
                "a losing claim writes nothing"
            );
        }

        let no_row = job_with(&mut conn, fixture, JobStatus::Processed, None);
        assert!(
            !QueryJobCallback::claim(&mut conn, no_row, at(1)).unwrap(),
            "a claim on a job with no callback loses"
        );
    }

    // Each claim runs on its own connection to one database file, so the two race
    // in SQLite itself, as two writers would, rather than taking turns on one lock.
    #[test]
    fn two_racing_claims_have_exactly_one_winner() {
        const ROUNDS: usize = 32;

        fn connect(path: &str) -> SqliteConnection {
            let mut conn = SqliteConnection::establish(path).unwrap();
            conn.batch_execute("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
                .unwrap();
            conn
        }

        fn race(conn: &mut SqliteConnection, barrier: &Barrier, job_ids: &[JobId]) -> Vec<bool> {
            job_ids
                .iter()
                .map(|&job_id| {
                    barrier.wait();
                    QueryJobCallback::claim(conn, job_id, at(1)).unwrap()
                })
                .collect()
        }

        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let mut conn = connect(path);
        conn.batch_execute("PRAGMA journal_mode = WAL").unwrap();
        run_migrations(&mut conn).unwrap();
        let fixture = create_job_fixture(&mut conn);
        let job_ids: Vec<JobId> =
            std::iter::repeat_with(|| callback(&mut conn, fixture, CallbackState::Pending))
                .take(ROUNDS)
                .collect();

        let mut first = connect(path);
        let mut second = connect(path);
        let barrier = Barrier::new(2);
        let (first_wins, second_wins) = thread::scope(|scope| {
            let first = scope.spawn(|| race(&mut first, &barrier, &job_ids));
            let second = scope.spawn(|| race(&mut second, &barrier, &job_ids));
            (first.join().unwrap(), second.join().unwrap())
        });

        for (round, (first, second)) in first_wins.iter().zip(&second_wins).enumerate() {
            assert!(
                first ^ second,
                "round {round}: first won {first}, second won {second}"
            );
        }
        for job_id in job_ids {
            assert_eq!(get(&mut conn, job_id).state, CallbackState::Delivering);
        }
    }

    #[test]
    fn an_attempt_counts_and_keeps_the_last_status() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, CallbackState::Delivering);

        assert!(QueryJobCallback::record_attempt(&mut conn, job_id, None, at(1)).unwrap());
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (1, None),
            "no response, no status"
        );
        assert_eq!(row.modified, at(1));

        let internal = StatusCode::INTERNAL_SERVER_ERROR;
        assert!(
            QueryJobCallback::record_attempt(&mut conn, job_id, Some(internal), at(2)).unwrap()
        );
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (2, Some(HttpStatus::from(internal)))
        );
        assert_eq!(row.modified, at(2));

        assert!(QueryJobCallback::record_attempt(&mut conn, job_id, None, at(3)).unwrap());
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (3, Some(HttpStatus::from(internal))),
            "an attempt with no response keeps the last status"
        );
        assert_eq!(row.modified, at(3));
        assert_eq!(
            row.state,
            CallbackState::Delivering,
            "an attempt is not an outcome"
        );

        let bad_gateway = StatusCode::BAD_GATEWAY;
        assert!(
            QueryJobCallback::record_attempt(&mut conn, job_id, Some(bad_gateway), at(4)).unwrap()
        );
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (4, Some(HttpStatus::from(bad_gateway))),
            "a later response replaces the last status"
        );
        assert_eq!(row.modified, at(4));
    }

    #[test]
    fn an_attempt_counts_only_while_delivering() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for state in [
            CallbackState::Pending,
            CallbackState::Delivered,
            CallbackState::Failed,
            CallbackState::Skipped,
        ] {
            let job_id = callback(&mut conn, fixture, state);
            assert!(
                !QueryJobCallback::record_attempt(&mut conn, job_id, Some(StatusCode::OK), at(1))
                    .unwrap(),
                "no attempt counts on {state:?}"
            );
            let row = get(&mut conn, job_id);
            assert_eq!(
                (row.attempts, row.status),
                (0, None),
                "{state:?} is untouched"
            );
            assert_eq!(row.modified, DateTime::TEST);
        }
    }

    #[test]
    fn finish_settles_the_callback_and_drops_its_sealed_request() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for (outcome, state) in [
            (CallbackOutcome::Delivered, CallbackState::Delivered),
            (CallbackOutcome::Failed, CallbackState::Failed),
        ] {
            let job_id = callback(&mut conn, fixture, CallbackState::Delivering);
            assert!(QueryJobCallback::finish(&mut conn, job_id, outcome, at(1)).unwrap());
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state);
            assert!(
                row.request.is_none(),
                "{outcome:?} drops the sealed request"
            );
            assert_eq!(row.modified, at(1));
        }
    }

    #[test]
    fn finish_and_release_leave_every_other_state_alone() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for state in [
            CallbackState::Pending,
            CallbackState::Delivered,
            CallbackState::Failed,
            CallbackState::Skipped,
        ] {
            let job_id = callback(&mut conn, fixture, state);
            let sealed = sealed_bytes(&get(&mut conn, job_id));
            for outcome in [CallbackOutcome::Delivered, CallbackOutcome::Failed] {
                assert!(
                    !QueryJobCallback::finish(&mut conn, job_id, outcome, at(1)).unwrap(),
                    "{outcome:?} does not settle {state:?}"
                );
            }
            assert!(
                !QueryJobCallback::release(&mut conn, job_id, at(1)).unwrap(),
                "{state:?} is not released"
            );
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state, "{state:?} is kept");
            assert_eq!(
                sealed_bytes(&row),
                sealed,
                "{state:?} keeps its sealed value"
            );
            assert_eq!(row.modified, DateTime::TEST, "{state:?} is not written");
        }
    }

    #[test]
    fn release_returns_a_delivering_callback_to_pending() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, CallbackState::Delivering);

        assert!(QueryJobCallback::release(&mut conn, job_id, at(1)).unwrap());
        let row = get(&mut conn, job_id);
        assert_eq!(row.state, CallbackState::Pending);
        assert!(row.request.is_some(), "a released callback can fire again");
        assert_eq!(row.modified, at(1));

        assert!(
            !QueryJobCallback::release(&mut conn, job_id, at(2)).unwrap(),
            "a pending callback is not released"
        );
        assert_eq!(get(&mut conn, job_id).modified, at(1));
        assert!(
            QueryJobCallback::claim(&mut conn, job_id, at(3)).unwrap(),
            "claimed again"
        );
    }

    #[test]
    fn a_job_has_at_most_one_callback() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = create_job(&mut conn, fixture, JobStatus::Pending);
        let sealed = sealed(JobUuid::new());
        let bytes = sealed.as_ref().to_vec();
        InsertJobCallback::pending(job_id, sealed, at(1))
            .insert(&mut conn)
            .unwrap();

        let error = InsertJobCallback::skipped(job_id, at(2))
            .insert(&mut conn)
            .unwrap_err();
        assert!(
            matches!(
                error,
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _
                )
            ),
            "a second callback for a job is refused: {error}"
        );

        let row = get(&mut conn, job_id);
        assert_eq!(row.state, CallbackState::Pending);
        assert_eq!(sealed_bytes(&row), Some(bytes), "the first request is kept");
        assert_eq!(row.modified, at(1));
    }

    // The attempt count is the delivery's budget across restarts, and the status is
    // the last answer a job shows, so only a recorded attempt may change either.
    #[test]
    fn only_an_attempt_changes_the_count_and_the_status() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = create_job(&mut conn, fixture, JobStatus::Processed);
        InsertJobCallback::pending(job_id, sealed(JobUuid::new()), DateTime::TEST)
            .insert(&mut conn)
            .unwrap();
        let bad_gateway = Some(HttpStatus::from(StatusCode::BAD_GATEWAY));

        assert!(QueryJobCallback::claim(&mut conn, job_id, at(1)).unwrap());
        assert_eq!(attempts_and_status(&mut conn, job_id), (0, None), "claim");

        for second in [2, 3] {
            assert!(
                QueryJobCallback::record_attempt(
                    &mut conn,
                    job_id,
                    Some(StatusCode::BAD_GATEWAY),
                    at(second)
                )
                .unwrap()
            );
        }
        assert_eq!(attempts_and_status(&mut conn, job_id), (2, bad_gateway));

        assert!(QueryJobCallback::release(&mut conn, job_id, at(4)).unwrap());
        assert_eq!(
            attempts_and_status(&mut conn, job_id),
            (2, bad_gateway),
            "release"
        );

        assert!(QueryJobCallback::claim(&mut conn, job_id, at(5)).unwrap());
        assert_eq!(
            attempts_and_status(&mut conn, job_id),
            (2, bad_gateway),
            "claim"
        );
        assert_eq!(
            QueryJobCallback::reset_delivering(&mut conn, at(6)).unwrap(),
            1
        );
        assert_eq!(
            attempts_and_status(&mut conn, job_id),
            (2, bad_gateway),
            "reset"
        );

        assert!(QueryJobCallback::claim(&mut conn, job_id, at(7)).unwrap());
        assert_eq!(
            attempts_and_status(&mut conn, job_id),
            (2, bad_gateway),
            "claim"
        );
        assert!(
            QueryJobCallback::finish(&mut conn, job_id, CallbackOutcome::Failed, at(8)).unwrap()
        );
        assert_eq!(
            attempts_and_status(&mut conn, job_id),
            (2, bad_gateway),
            "finish"
        );
    }

    #[test]
    fn the_startup_reset_moves_every_delivering_row_and_no_other() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let delivering = [
            callback(&mut conn, fixture, CallbackState::Delivering),
            callback(&mut conn, fixture, CallbackState::Delivering),
        ];
        let others = [
            (
                callback(&mut conn, fixture, CallbackState::Pending),
                CallbackState::Pending,
            ),
            (
                callback(&mut conn, fixture, CallbackState::Delivered),
                CallbackState::Delivered,
            ),
            (
                callback(&mut conn, fixture, CallbackState::Failed),
                CallbackState::Failed,
            ),
            (
                callback(&mut conn, fixture, CallbackState::Skipped),
                CallbackState::Skipped,
            ),
        ];

        assert_eq!(
            QueryJobCallback::reset_delivering(&mut conn, at(1)).unwrap(),
            2
        );

        for job_id in delivering {
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, CallbackState::Pending);
            assert!(row.request.is_some(), "a reset callback can fire again");
            assert_eq!(row.modified, at(1));
        }
        for (job_id, state) in others {
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state, "{state:?} is not reset");
            assert_eq!(row.modified, DateTime::TEST, "{state:?} is not written");
        }
    }

    #[test]
    fn pending_callbacks_are_due_only_on_terminal_jobs() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let mut due = Vec::new();
        for status in [
            JobStatus::Pending,
            JobStatus::Claimed,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Processed,
            JobStatus::Failed,
            JobStatus::Canceled,
            JobStatus::Unknown,
        ] {
            let pending = job_with(&mut conn, fixture, status, Some(CallbackState::Pending));
            if matches!(
                status,
                JobStatus::Processed | JobStatus::Failed | JobStatus::Canceled
            ) {
                due.push(pending);
            }
            for state in [
                CallbackState::Delivering,
                CallbackState::Delivered,
                CallbackState::Failed,
                CallbackState::Skipped,
            ] {
                job_with(&mut conn, fixture, status, Some(state));
            }
            job_with(&mut conn, fixture, status, None);
        }

        assert_eq!(
            QueryJobCallback::pending_on_terminal_jobs(&mut conn).unwrap(),
            due
        );
    }

    #[test]
    fn deleting_a_job_deletes_its_callback() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let deleted = callback(&mut conn, fixture, CallbackState::Pending);
        let kept = callback(&mut conn, fixture, CallbackState::Pending);

        let rows = diesel::delete(schema::job::table.filter(schema::job::id.eq(deleted)))
            .execute(&mut conn)
            .unwrap();
        assert_eq!(rows, 1, "the job is deleted");

        let remaining: Vec<JobId> = schema::job_callback::table
            .select(schema::job_callback::job_id)
            .load(&mut conn)
            .unwrap();
        assert_eq!(remaining, vec![kept]);
    }
}
