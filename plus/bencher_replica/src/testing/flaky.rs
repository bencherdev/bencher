//! Fault-injection storage wrapper: wraps any [`ReplicaStorage`] with a
//! scripted [`FailurePlan`] and records an operation journal for assertions
//! (retry counts, no-double-ship, ordering).
//!
//! Every operation, passthrough or injected, is journaled with its key and
//! outcome. A fired rule injects either BEFORE the wrapped call (the backend
//! is never touched) or, in ambiguous-success mode, AFTER it (the backend
//! applies the operation but the caller still sees an injected error: the
//! lost-200 case that exercises retry idempotency). The plan and journal are
//! shared behind `Arc` so in-flight [`FlakyMultipart`] uploads consult the
//! same live plan.

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use bytes::Bytes;

use crate::storage::{MultipartUpload, ReplicaStorage, StorageError};

/// Fault-injection wrapper. Construct with [`FlakyStorage::new`], mutate the
/// live plan via [`FlakyStorage::set_plan`]/[`FlakyStorage::heal`], and read
/// the journal via [`FlakyStorage::journal`].
pub struct FlakyStorage {
    inner: Box<ReplicaStorage>,
    shared: Shared,
}

impl FlakyStorage {
    #[must_use]
    pub fn new(inner: ReplicaStorage, plan: FailurePlan) -> Self {
        Self {
            inner: Box::new(inner),
            shared: Shared {
                plan: Arc::new(Mutex::new(plan)),
                journal: Arc::new(Mutex::new(Vec::new())),
            },
        }
    }

    /// Replace the live failure plan (e.g. heal after an outage).
    pub fn set_plan(&self, plan: FailurePlan) {
        *self.shared.lock_plan() = plan;
    }

    /// Remove all failure rules: every subsequent operation passes through.
    pub fn heal(&self) {
        self.set_plan(FailurePlan::new());
    }

    /// Snapshot of the operation journal.
    #[must_use]
    pub fn journal(&self) -> Vec<(Op, OpOutcome)> {
        self.shared.lock_journal().clone()
    }

    // Every method boxes the inner future (`ReplicaStorage` methods can
    // recurse back into this wrapper, so the cycle must be broken with a heap
    // allocation) and routes it through `Shared::guard`, which applies the
    // plan (before/after/none) and journals exactly once.
    pub(crate) async fn put(&self, key: &str, bytes: Bytes) -> Result<(), StorageError> {
        self.shared
            .guard(OpKind::Put, key, Box::pin(self.inner.put(key, bytes)))
            .await
    }

    pub(crate) async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        self.shared
            .guard(OpKind::Get, key, Box::pin(self.inner.get(key)))
            .await
    }

    pub(crate) async fn get_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, StorageError> {
        self.shared
            .guard(OpKind::GetStream, key, Box::pin(self.inner.get_stream(key)))
            .await
    }

    pub(crate) async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.shared
            .guard(OpKind::List, prefix, Box::pin(self.inner.list(prefix)))
            .await
    }

    pub(crate) async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.shared
            .guard(
                OpKind::ListDirs,
                prefix,
                Box::pin(self.inner.list_dirs(prefix)),
            )
            .await
    }

    pub(crate) async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.shared
            .guard(OpKind::Delete, key, Box::pin(self.inner.delete(key)))
            .await
    }

    pub(crate) async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        self.shared
            .guard(
                OpKind::DeletePrefix,
                prefix,
                Box::pin(self.inner.delete_prefix(prefix)),
            )
            .await
    }

    pub(crate) async fn start_multipart(&self, key: &str) -> Result<FlakyMultipart, StorageError> {
        let inner = self
            .shared
            .guard(
                OpKind::MultipartStart,
                key,
                Box::pin(self.inner.start_multipart(key)),
            )
            .await?;
        Ok(FlakyMultipart {
            inner: Box::new(inner),
            key: key.to_owned(),
            shared: self.shared.clone(),
        })
    }
}

/// Multipart wrapper that consults the parent plan on every operation. An
/// injected `finish` drops the inner upload unfinished, so the object never
/// becomes visible (matching a real failure).
pub struct FlakyMultipart {
    inner: Box<MultipartUpload>,
    key: String,
    shared: Shared,
}

impl FlakyMultipart {
    pub(crate) async fn write_part(&mut self, bytes: Bytes) -> Result<(), StorageError> {
        self.shared
            .guard(
                OpKind::MultipartWrite,
                &self.key,
                Box::pin(self.inner.write_part(bytes)),
            )
            .await
    }

    pub(crate) async fn finish(self) -> Result<(), StorageError> {
        let Self { inner, key, shared } = self;
        shared
            .guard(OpKind::MultipartFinish, &key, Box::pin(inner.finish()))
            .await
    }

    pub(crate) async fn abort(self) -> Result<(), StorageError> {
        let Self { inner, key, shared } = self;
        shared
            .guard(OpKind::MultipartAbort, &key, Box::pin(inner.abort()))
            .await
    }
}

/// The plan and journal handles shared between the storage wrapper and its
/// in-flight multipart uploads.
#[derive(Clone)]
struct Shared {
    plan: Arc<Mutex<FailurePlan>>,
    journal: Arc<Mutex<Vec<(Op, OpOutcome)>>>,
}

impl Shared {
    /// Run one wrapped operation under the live plan, journaling exactly once.
    ///
    /// - No rule fires: run the backend op and journal its real outcome.
    /// - A rule fires [`Timing::Before`]: skip the backend, journal `Injected`,
    ///   return the injected error (the op had no effect).
    /// - A rule fires [`Timing::After`]: run the backend op, discard its
    ///   result, journal `InjectedAfter`, return the injected error (the
    ///   backend applied the write but the caller sees failure).
    async fn guard<T, Fut>(&self, kind: OpKind, key: &str, op: Fut) -> Result<T, StorageError>
    where
        Fut: Future<Output = Result<T, StorageError>>,
    {
        // Drop the plan lock before awaiting: the guard is not held across the
        // backend call.
        let timing = self.lock_plan().evaluate(kind, key);
        match timing {
            Some(Timing::Before) => {
                self.record(kind, key, OpOutcome::Injected);
                Err(injected(kind, key))
            },
            Some(Timing::After) => {
                drop(op.await);
                self.record(kind, key, OpOutcome::InjectedAfter);
                Err(injected(kind, key))
            },
            None => {
                let result = op.await;
                self.record_result(kind, key, &result);
                result
            },
        }
    }

    fn record_result<T>(&self, kind: OpKind, key: &str, result: &Result<T, StorageError>) {
        let outcome = match result {
            Ok(_) => OpOutcome::Ok,
            Err(error) => OpOutcome::Err(error.to_string()),
        };
        self.record(kind, key, outcome);
    }

    fn record(&self, kind: OpKind, key: &str, outcome: OpOutcome) {
        self.lock_journal().push((
            Op {
                kind,
                key: key.to_owned(),
            },
            outcome,
        ));
    }

    fn lock_plan(&self) -> std::sync::MutexGuard<'_, FailurePlan> {
        self.plan.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_journal(&self) -> std::sync::MutexGuard<'_, Vec<(Op, OpOutcome)>> {
        self.journal.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The injected error for one operation.
fn injected(kind: OpKind, key: &str) -> StorageError {
    StorageError::Injected {
        op: kind.as_str(),
        key: key.to_owned(),
    }
}

/// When a fired rule injects its failure relative to the wrapped backend call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Timing {
    /// Fail before touching the backend: the operation has no effect.
    Before,
    /// Perform the operation, then return an injected error: the backend
    /// applied the write but the caller sees failure (ambiguous success).
    After,
}

/// A scripted failure plan. Rules are evaluated per operation in the order
/// they were added; the first matching rule fires (and, for bounded rules,
/// is consumed).
#[derive(Debug, Default)]
pub struct FailurePlan {
    rules: Vec<Rule>,
}

impl FailurePlan {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fail the `n`th (1-indexed) operation of `kind` observed from now on.
    #[must_use]
    pub fn fail_nth(mut self, kind: OpKind, n: u64) -> Self {
        self.rules.push(Rule::Nth {
            kind,
            n,
            seen: 0,
            timing: Timing::Before,
        });
        self
    }

    /// Fail the next `times` operations whose key contains `key_contains`
    /// (any kind when `kind` is `None`).
    #[must_use]
    pub fn fail_matching(mut self, kind: Option<OpKind>, key_contains: &str, times: u64) -> Self {
        self.rules.push(Rule::Matching {
            kind,
            key_contains: key_contains.to_owned(),
            times,
            timing: Timing::Before,
        });
        self
    }

    /// Fail every operation of `kind` until healed via
    /// [`FlakyStorage::heal`] or [`FlakyStorage::set_plan`].
    #[must_use]
    pub fn fail_all(mut self, kind: OpKind) -> Self {
        self.rules.push(Rule::All {
            kind,
            timing: Timing::Before,
        });
        self
    }

    /// Make the most recently added rule fire in ambiguous-success mode: the
    /// backend operation runs and THEN an injected error is returned (the
    /// lost-200 case that exercises retry idempotency). The operation is
    /// journaled as [`OpOutcome::InjectedAfter`]. A no-op when no rule has been
    /// added yet.
    #[must_use]
    pub fn after(mut self) -> Self {
        if let Some(rule) = self.rules.last_mut() {
            rule.set_timing(Timing::After);
        }
        self
    }

    /// Evaluate the plan for one operation: the first matching rule fires,
    /// bounded rules are consumed once exhausted. Returns the fired rule's
    /// [`Timing`] (or `None` when no rule fires). Rules after the firing rule
    /// are untouched (their counters do not advance).
    fn evaluate(&mut self, kind: OpKind, key: &str) -> Option<Timing> {
        let mut fired_index = None;
        for (index, rule) in self.rules.iter_mut().enumerate() {
            let fired = match rule {
                Rule::Nth {
                    kind: rule_kind,
                    n,
                    seen,
                    ..
                } => {
                    if *rule_kind == kind {
                        *seen += 1;
                        *seen == *n
                    } else {
                        false
                    }
                },
                Rule::Matching {
                    kind: rule_kind,
                    key_contains,
                    times,
                    ..
                } => {
                    if rule_kind.is_none_or(|rule_kind| rule_kind == kind)
                        && key.contains(key_contains.as_str())
                        && *times > 0
                    {
                        *times -= 1;
                        true
                    } else {
                        false
                    }
                },
                Rule::All {
                    kind: rule_kind, ..
                } => *rule_kind == kind,
            };
            if fired {
                fired_index = Some(index);
                break;
            }
        }
        let index = fired_index?;
        let timing = self.rules.get(index).map(Rule::timing)?;
        let consumed = match self.rules.get(index) {
            Some(Rule::Nth { .. }) => true,
            Some(Rule::Matching { times, .. }) => *times == 0,
            Some(Rule::All { .. }) | None => false,
        };
        if consumed {
            self.rules.remove(index);
        }
        Some(timing)
    }
}

#[derive(Debug)]
enum Rule {
    /// Fail the `n`th (1-indexed) operation of `kind` observed from now on.
    Nth {
        kind: OpKind,
        n: u64,
        seen: u64,
        timing: Timing,
    },
    /// Fail the next `times` operations whose key contains `key_contains`
    /// (any kind when `kind` is `None`).
    Matching {
        kind: Option<OpKind>,
        key_contains: String,
        times: u64,
        timing: Timing,
    },
    /// Fail every operation of `kind` until the rule is removed via
    /// [`FlakyStorage::heal`].
    All { kind: OpKind, timing: Timing },
}

impl Rule {
    fn timing(&self) -> Timing {
        match self {
            Self::Nth { timing, .. } | Self::Matching { timing, .. } | Self::All { timing, .. } => {
                *timing
            },
        }
    }

    fn set_timing(&mut self, timing: Timing) {
        match self {
            Self::Nth { timing: slot, .. }
            | Self::Matching { timing: slot, .. }
            | Self::All { timing: slot, .. } => *slot = timing,
        }
    }
}

/// The kind of storage operation, for matching in failure plans and the
/// journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Put,
    Get,
    GetStream,
    List,
    ListDirs,
    Delete,
    DeletePrefix,
    MultipartStart,
    MultipartWrite,
    MultipartFinish,
    MultipartAbort,
}

impl OpKind {
    /// The stable name used in [`StorageError::Injected`] messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::Get => "get",
            Self::GetStream => "get_stream",
            Self::List => "list",
            Self::ListDirs => "list_dirs",
            Self::Delete => "delete",
            Self::DeletePrefix => "delete_prefix",
            Self::MultipartStart => "multipart_start",
            Self::MultipartWrite => "multipart_write",
            Self::MultipartFinish => "multipart_finish",
            Self::MultipartAbort => "multipart_abort",
        }
    }
}

/// One recorded storage operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    pub kind: OpKind,
    pub key: String,
}

/// The outcome recorded in the journal for one operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpOutcome {
    Ok,
    Injected,
    /// The backend applied the operation but an injected error was returned
    /// afterward (ambiguous success / lost-200).
    InjectedAfter,
    Err(String),
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use pretty_assertions::assert_eq;

    use crate::local::LocalStorage;

    use super::*;

    fn flaky(tmp: &tempfile::TempDir, plan: FailurePlan) -> FlakyStorage {
        let root = Utf8Path::from_path(tmp.path())
            .expect("tempdir path is UTF-8")
            .to_path_buf();
        FlakyStorage::new(ReplicaStorage::Local(LocalStorage::new(root)), plan)
    }

    fn op(kind: OpKind, key: &str) -> Op {
        Op {
            kind,
            key: key.to_owned(),
        }
    }

    fn assert_injected(result: Result<(), StorageError>, expected_op: &str, context: &str) {
        match result {
            Err(StorageError::Injected { op, .. }) => {
                assert_eq!(op, expected_op, "{context}: wrong injected op");
            },
            Err(error) => panic!("{context}: unexpected error: {error}"),
            Ok(()) => panic!("{context}: expected an injected failure"),
        }
    }

    #[tokio::test]
    async fn passthrough_records_ok_in_journal() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new());
        storage
            .put("wal/seg1", Bytes::from_static(b"1"))
            .await
            .expect("put failed");
        let got = storage.get("wal/seg1").await.expect("get failed");
        assert_eq!(got.as_ref(), b"1".as_slice(), "passthrough value");
        assert_eq!(
            storage.journal(),
            vec![
                (op(OpKind::Put, "wal/seg1"), OpOutcome::Ok),
                (op(OpKind::Get, "wal/seg1"), OpOutcome::Ok),
            ],
            "journal must record passthrough operations"
        );
    }

    #[tokio::test]
    async fn passthrough_records_backend_error() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new());
        let error = storage.get("missing").await.expect_err("get must fail");
        assert!(
            matches!(&error, StorageError::NotFound { key } if key == "missing"),
            "NotFound must pass through unchanged, got: {error}"
        );
        assert_eq!(
            storage.journal(),
            vec![(
                op(OpKind::Get, "missing"),
                OpOutcome::Err("Object not found: missing".to_owned())
            )],
            "journal must record backend errors"
        );
    }

    #[tokio::test]
    async fn fail_nth_fires_once_then_consumed() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new().fail_nth(OpKind::Put, 2));
        storage
            .put("a", Bytes::from_static(b"1"))
            .await
            .expect("first put must pass");
        assert_injected(
            storage.put("b", Bytes::from_static(b"2")).await,
            "put",
            "second put",
        );
        storage
            .put("c", Bytes::from_static(b"3"))
            .await
            .expect("third put must pass: the rule is consumed");
        // The injected put never reached the backend.
        let error = storage.get("b").await.expect_err("b must not exist");
        assert!(
            matches!(error, StorageError::NotFound { .. }),
            "injected put must not write"
        );
    }

    #[tokio::test]
    async fn fail_matching_bounded_times_consumed() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new().fail_matching(None, "wal/", 2));
        assert_injected(
            storage.put("wal/1", Bytes::from_static(b"1")).await,
            "put",
            "first wal put",
        );
        assert_injected(
            storage.put("wal/2", Bytes::from_static(b"2")).await,
            "put",
            "second wal put",
        );
        storage
            .put("wal/3", Bytes::from_static(b"3"))
            .await
            .expect("third wal put must pass: rule exhausted");
        storage
            .put("other", Bytes::from_static(b"4"))
            .await
            .expect("non-matching key must pass");
    }

    #[tokio::test]
    async fn fail_matching_filters_by_kind() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(
            &tmp,
            FailurePlan::new().fail_matching(Some(OpKind::Get), "x", 1),
        );
        storage
            .put("x", Bytes::from_static(b"1"))
            .await
            .expect("put must pass: rule only matches Get");
        let error = storage.get("x").await.expect_err("get must be injected");
        assert!(
            matches!(error, StorageError::Injected { op: "get", .. }),
            "expected injected get"
        );
        let got = storage.get("x").await.expect("second get must pass");
        assert_eq!(got.as_ref(), b"1".as_slice(), "value after heal");
    }

    #[tokio::test]
    async fn fail_all_persists_until_heal() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new().fail_all(OpKind::List));
        storage
            .put("k", Bytes::from_static(b"1"))
            .await
            .expect("put must pass");
        for attempt in 0..3 {
            let result = storage.list("").await;
            assert!(
                matches!(result, Err(StorageError::Injected { op: "list", .. })),
                "list attempt {attempt} must stay injected"
            );
        }
        storage.heal();
        let keys = storage.list("").await.expect("healed list must pass");
        assert_eq!(keys, vec!["k".to_owned()], "healed list content");
    }

    #[tokio::test]
    async fn set_plan_replaces_live_plan() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new());
        storage
            .put("k", Bytes::from_static(b"1"))
            .await
            .expect("put must pass");
        storage.set_plan(FailurePlan::new().fail_all(OpKind::Get));
        let error = storage.get("k").await.expect_err("get must be injected");
        assert!(
            matches!(error, StorageError::Injected { op: "get", .. }),
            "expected injected get after set_plan"
        );
        storage.set_plan(FailurePlan::new());
        let got = storage.get("k").await.expect("get after reset must pass");
        assert_eq!(got.as_ref(), b"1".as_slice(), "value after reset");
    }

    #[tokio::test]
    async fn first_matching_rule_wins_and_later_counters_do_not_advance() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        // Rule 1 matches only key "k" once; rule 2 fails the first Put it
        // observes. If evaluation stopped at rule 1 for the first put, rule
        // 2 must fire on the SECOND put (its counter did not advance).
        let storage = flaky(
            &tmp,
            FailurePlan::new()
                .fail_matching(None, "k", 1)
                .fail_nth(OpKind::Put, 1),
        );
        assert_injected(
            storage.put("k", Bytes::from_static(b"1")).await,
            "put",
            "first put (rule 1)",
        );
        assert_injected(
            storage.put("z", Bytes::from_static(b"2")).await,
            "put",
            "second put (rule 2)",
        );
        storage
            .put("w", Bytes::from_static(b"3"))
            .await
            .expect("third put must pass: both rules consumed");
    }

    #[tokio::test]
    async fn multipart_write_consults_parent_plan() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new().fail_nth(OpKind::MultipartWrite, 2));
        let mut upload = storage
            .start_multipart("snap/db.zst")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"one"))
            .await
            .expect("first write must pass");
        assert_injected(
            upload.write_part(Bytes::from_static(b"two")).await,
            "multipart_write",
            "second write",
        );
        upload
            .write_part(Bytes::from_static(b"three"))
            .await
            .expect("third write must pass");
        upload.abort().await.expect("abort failed");
        assert_eq!(
            storage.journal(),
            vec![
                (op(OpKind::MultipartStart, "snap/db.zst"), OpOutcome::Ok),
                (op(OpKind::MultipartWrite, "snap/db.zst"), OpOutcome::Ok),
                (
                    op(OpKind::MultipartWrite, "snap/db.zst"),
                    OpOutcome::Injected
                ),
                (op(OpKind::MultipartWrite, "snap/db.zst"), OpOutcome::Ok),
                (op(OpKind::MultipartAbort, "snap/db.zst"), OpOutcome::Ok),
            ],
            "multipart journal must record every operation with its outcome"
        );
    }

    #[tokio::test]
    async fn multipart_finish_injection_leaves_object_invisible() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(
            &tmp,
            FailurePlan::new().fail_nth(OpKind::MultipartFinish, 1),
        );
        let mut upload = storage
            .start_multipart("snap/db.zst")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"data"))
            .await
            .expect("write failed");
        assert_injected(upload.finish().await, "multipart_finish", "finish");
        let error = storage
            .get("snap/db.zst")
            .await
            .expect_err("object must not exist");
        assert!(
            matches!(error, StorageError::NotFound { .. }),
            "injected finish must leave the object invisible"
        );
        let keys = storage.list("").await.expect("list failed");
        assert_eq!(keys, Vec::<String>::new(), "no objects after failed finish");
    }

    #[tokio::test]
    async fn after_mode_applies_operation_then_reports_failure() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = flaky(&tmp, FailurePlan::new().fail_nth(OpKind::Put, 1).after());
        // The put reports failure to the caller...
        assert_injected(
            storage.put("k", Bytes::from_static(b"v")).await,
            "put",
            "ambiguous put",
        );
        // ...but the backend actually applied it: a clean get finds the object.
        let got = storage
            .get("k")
            .await
            .expect("after-mode put must have landed server-side");
        assert_eq!(got.as_ref(), b"v".as_slice(), "after-mode put lands");
        assert_eq!(
            storage.journal(),
            vec![
                (op(OpKind::Put, "k"), OpOutcome::InjectedAfter),
                (op(OpKind::Get, "k"), OpOutcome::Ok),
            ],
            "an after-mode injection journals distinctly as InjectedAfter"
        );
    }

    #[tokio::test]
    async fn paired_operations_have_distinct_op_kinds() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        // A plan that fails `List` must NOT catch `list_dirs`: distinct kinds.
        let storage = flaky(&tmp, FailurePlan::new().fail_all(OpKind::List));
        storage
            .put("gen/g/x", Bytes::from_static(b"1"))
            .await
            .expect("put must pass");
        assert!(
            matches!(
                storage.list("").await,
                Err(StorageError::Injected { op: "list", .. })
            ),
            "list is failed"
        );
        let dirs = storage
            .list_dirs("")
            .await
            .expect("list_dirs is a distinct kind and passes through");
        assert_eq!(dirs, vec!["gen".to_owned()], "list_dirs result");
        storage
            .get_stream("gen/g/x")
            .await
            .map(drop)
            .expect("get_stream is a distinct kind");
        storage
            .delete_prefix("gen/")
            .await
            .expect("delete_prefix is a distinct kind");
        let kinds: Vec<OpKind> = storage
            .journal()
            .into_iter()
            .map(|(op, _)| op.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                OpKind::Put,
                OpKind::List,
                OpKind::ListDirs,
                OpKind::GetStream,
                OpKind::DeletePrefix,
            ],
            "each paired operation journals under its own kind"
        );
    }
}
