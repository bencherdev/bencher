use bencher_callback::SealedRequest;
use bencher_json::{
    DateTime,
    runner::{CALLBACK_JOB_STATUSES, JobCallbackState, JsonJobCallback},
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
    backend::Backend,
    deserialize::{self, FromSql},
    dsl::Eq,
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

#[derive(Debug, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_callback_table)]
pub struct QueryJobCallback {
    pub job_id: JobId,
    pub request: Option<SealedRequest>,
    pub state: JobCallbackState,
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

    /// Count an attempt on a pending callback, and keep its status if the receiver answered.
    /// With an outcome, the attempt ends the delivery, and the same update settles the callback.
    pub fn record_attempt(
        conn: &mut DbConnection,
        job_id: JobId,
        status: Option<StatusCode>,
        outcome: Option<CallbackOutcome>,
        now: DateTime,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::job_callback::table
                .filter(schema::job_callback::job_id.eq(job_id))
                .filter(schema::job_callback::state.eq(JobCallbackState::Pending)),
        )
        .set((
            schema::job_callback::attempts.eq(schema::job_callback::attempts + 1),
            status.map(|status| schema::job_callback::status.eq(HttpStatus::from(status))),
            outcome.map(settle),
            schema::job_callback::modified.eq(now),
        ))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// Settle a pending callback without an attempt.
    pub fn finish(
        conn: &mut DbConnection,
        job_id: JobId,
        outcome: CallbackOutcome,
        now: DateTime,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::job_callback::table
                .filter(schema::job_callback::job_id.eq(job_id))
                .filter(schema::job_callback::state.eq(JobCallbackState::Pending)),
        )
        .set((settle(outcome), schema::job_callback::modified.eq(now)))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// The jobs whose pending callback is due, since each has reached a terminal status.
    pub fn pending_on_terminal_jobs(conn: &mut DbConnection) -> QueryResult<Vec<JobId>> {
        schema::job_callback::table
            .inner_join(schema::job::table)
            .filter(schema::job_callback::state.eq(JobCallbackState::Pending))
            .filter(schema::job::status.eq_any(CALLBACK_JOB_STATUSES))
            .order(schema::job_callback::job_id.asc())
            .select(schema::job_callback::job_id)
            .load(conn)
    }

    /// The callbacks waiting to be delivered, which the partial index answers.
    pub fn count_pending(conn: &mut DbConnection) -> QueryResult<i64> {
        schema::job_callback::table
            .filter(schema::job_callback::state.eq(JobCallbackState::Pending))
            .count()
            .get_result(conn)
    }
}

/// The outcome's state, and no sealed request, which a settled callback never needs again.
fn settle(
    outcome: CallbackOutcome,
) -> (
    Eq<schema::job_callback::state, JobCallbackState>,
    Eq<schema::job_callback::request, Option<SealedRequest>>,
) {
    (
        schema::job_callback::state.eq(JobCallbackState::from(outcome)),
        schema::job_callback::request.eq(None),
    )
}

/// A callback's state and last status, without its sealed request, so a job list never loads one.
#[derive(Debug, Clone, Copy, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_callback_table)]
pub struct QueryJobCallbackView {
    pub state: JobCallbackState,
    pub status: Option<HttpStatus>,
}

impl From<QueryJobCallbackView> for JsonJobCallback {
    fn from(view: QueryJobCallbackView) -> Self {
        let QueryJobCallbackView { state, status } = view;
        Self {
            state,
            status: status.map(u16::from),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_callback_table)]
pub struct InsertJobCallback {
    job_id: JobId,
    request: Option<SealedRequest>,
    state: JobCallbackState,
    created: DateTime,
    modified: DateTime,
}

impl InsertJobCallback {
    pub fn pending(job_id: JobId, request: SealedRequest, now: DateTime) -> Self {
        Self {
            job_id,
            request: Some(request),
            state: JobCallbackState::Pending,
            created: now,
            modified: now,
        }
    }

    /// A callback that is accepted but never sent, so nothing is sealed.
    pub fn skipped(job_id: JobId, now: DateTime) -> Self {
        Self {
            job_id,
            request: None,
            state: JobCallbackState::Skipped,
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

/// How a pending callback ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackOutcome {
    Delivered,
    Failed,
}

impl From<CallbackOutcome> for JobCallbackState {
    fn from(outcome: CallbackOutcome) -> Self {
        match outcome {
            CallbackOutcome::Delivered => Self::Delivered,
            CallbackOutcome::Failed => Self::Failed,
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
    use bencher_callback::{CallbackKey, SealedRequest};
    use bencher_json::{DateTime, JobStatus, JobUuid, Secret, runner::JobCallbackState};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use http::StatusCode;
    use pretty_assertions::assert_eq;

    use super::{CallbackOutcome, HttpStatus, InsertJobCallback, QueryJobCallback};
    use crate::{
        context::DbConnection,
        model::runner::JobId,
        schema,
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
        state: Option<JobCallbackState>,
    ) -> JobId {
        let job_id = create_job(conn, fixture, status);
        let Some(state) = state else {
            return job_id;
        };
        if state == JobCallbackState::Skipped {
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

    fn callback(conn: &mut DbConnection, fixture: JobFixture, state: JobCallbackState) -> JobId {
        job_with(conn, fixture, JobStatus::Processed, Some(state))
    }

    fn set_state(conn: &mut DbConnection, job_id: JobId, state: JobCallbackState) {
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

    fn raw_state(conn: &mut DbConnection, job_id: JobId) -> i32 {
        schema::job_callback::table
            .filter(schema::job_callback::job_id.eq(job_id))
            .select(schema::job_callback::state)
            .first(conn)
            .unwrap()
    }

    // The migration's partial index names Pending by its integer,
    // and a stored integer outlives any release, so the mapping never moves.
    #[test]
    fn state_integers_are_stable() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, JobCallbackState::Pending);
        for (state, integer) in [
            (JobCallbackState::Pending, 0),
            (JobCallbackState::Delivered, 2),
            (JobCallbackState::Failed, 3),
            (JobCallbackState::Skipped, 4),
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
        let bad_state = callback(&mut conn, fixture, JobCallbackState::Pending);
        let bad_status = callback(&mut conn, fixture, JobCallbackState::Pending);
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
        assert_eq!(row.state, JobCallbackState::Pending);
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
        assert_eq!(row.state, JobCallbackState::Skipped);
        assert!(row.request.is_none(), "a skipped row seals nothing");
        assert_eq!(row.attempts, 0);
        assert_eq!(row.status, None);
        assert_eq!(row.modified, at(1));
    }

    #[test]
    fn an_attempt_counts_and_keeps_the_last_status() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let job_id = callback(&mut conn, fixture, JobCallbackState::Pending);

        assert!(QueryJobCallback::record_attempt(&mut conn, job_id, None, None, at(1)).unwrap());
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (1, None),
            "no response, no status"
        );
        assert_eq!(row.modified, at(1));

        let internal = StatusCode::INTERNAL_SERVER_ERROR;
        assert!(
            QueryJobCallback::record_attempt(&mut conn, job_id, Some(internal), None, at(2))
                .unwrap()
        );
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (2, Some(HttpStatus::from(internal)))
        );
        assert_eq!(row.modified, at(2));

        assert!(QueryJobCallback::record_attempt(&mut conn, job_id, None, None, at(3)).unwrap());
        let row = get(&mut conn, job_id);
        assert_eq!(
            (row.attempts, row.status),
            (3, Some(HttpStatus::from(internal))),
            "an attempt with no response keeps the last status"
        );
        assert_eq!(row.modified, at(3));
        assert_eq!(
            row.state,
            JobCallbackState::Pending,
            "an attempt without an outcome settles nothing"
        );
        assert!(row.request.is_some(), "a retry keeps the sealed request");

        let bad_gateway = StatusCode::BAD_GATEWAY;
        assert!(
            QueryJobCallback::record_attempt(&mut conn, job_id, Some(bad_gateway), None, at(4))
                .unwrap()
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
    fn an_attempt_with_an_outcome_settles_in_the_same_update() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for (status, outcome, state) in [
            (
                Some(StatusCode::NO_CONTENT),
                CallbackOutcome::Delivered,
                JobCallbackState::Delivered,
            ),
            (
                Some(StatusCode::NOT_FOUND),
                CallbackOutcome::Failed,
                JobCallbackState::Failed,
            ),
            (None, CallbackOutcome::Failed, JobCallbackState::Failed),
        ] {
            let job_id = callback(&mut conn, fixture, JobCallbackState::Pending);
            assert!(
                QueryJobCallback::record_attempt(&mut conn, job_id, status, Some(outcome), at(1))
                    .unwrap()
            );
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state, "{outcome:?}");
            assert_eq!(
                (row.attempts, row.status),
                (1, status.map(HttpStatus::from)),
                "{outcome:?}"
            );
            assert!(
                row.request.is_none(),
                "{outcome:?} drops the sealed request"
            );
            assert_eq!(row.modified, at(1));
        }
    }

    #[test]
    fn finish_settles_the_callback_and_keeps_its_attempts() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let bad_gateway = Some(HttpStatus::from(StatusCode::BAD_GATEWAY));
        for (outcome, state) in [
            (CallbackOutcome::Delivered, JobCallbackState::Delivered),
            (CallbackOutcome::Failed, JobCallbackState::Failed),
        ] {
            let job_id = callback(&mut conn, fixture, JobCallbackState::Pending);
            assert!(
                QueryJobCallback::record_attempt(
                    &mut conn,
                    job_id,
                    Some(StatusCode::BAD_GATEWAY),
                    None,
                    at(1)
                )
                .unwrap()
            );
            assert!(QueryJobCallback::finish(&mut conn, job_id, outcome, at(2)).unwrap());
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state);
            assert!(
                row.request.is_none(),
                "{outcome:?} drops the sealed request"
            );
            assert_eq!(
                (row.attempts, row.status),
                (1, bad_gateway),
                "only an attempt changes the count and the status"
            );
            assert_eq!(row.modified, at(2));
        }
    }

    #[test]
    fn only_a_pending_callback_is_written() {
        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        for state in [
            JobCallbackState::Delivered,
            JobCallbackState::Failed,
            JobCallbackState::Skipped,
        ] {
            let job_id = callback(&mut conn, fixture, state);
            let sealed = sealed_bytes(&get(&mut conn, job_id));
            for outcome in [None, Some(CallbackOutcome::Delivered)] {
                assert!(
                    !QueryJobCallback::record_attempt(
                        &mut conn,
                        job_id,
                        Some(StatusCode::OK),
                        outcome,
                        at(1)
                    )
                    .unwrap(),
                    "no attempt counts on {state:?}"
                );
            }
            for outcome in [CallbackOutcome::Delivered, CallbackOutcome::Failed] {
                assert!(
                    !QueryJobCallback::finish(&mut conn, job_id, outcome, at(1)).unwrap(),
                    "{outcome:?} does not settle {state:?}"
                );
            }
            let row = get(&mut conn, job_id);
            assert_eq!(row.state, state, "{state:?} is kept");
            assert_eq!(
                sealed_bytes(&row),
                sealed,
                "{state:?} keeps its sealed value"
            );
            assert_eq!(
                (row.attempts, row.status),
                (0, None),
                "{state:?} is untouched"
            );
            assert_eq!(row.modified, DateTime::TEST, "{state:?} is not written");
        }
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
        assert_eq!(row.state, JobCallbackState::Pending);
        assert_eq!(sealed_bytes(&row), Some(bytes), "the first request is kept");
        assert_eq!(row.modified, at(1));
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
            let pending = job_with(&mut conn, fixture, status, Some(JobCallbackState::Pending));
            if matches!(
                status,
                JobStatus::Processed | JobStatus::Failed | JobStatus::Canceled
            ) {
                due.push(pending);
            }
            for state in [
                JobCallbackState::Delivered,
                JobCallbackState::Failed,
                JobCallbackState::Skipped,
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
        let deleted = callback(&mut conn, fixture, JobCallbackState::Pending);
        let kept = callback(&mut conn, fixture, JobCallbackState::Pending);

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
