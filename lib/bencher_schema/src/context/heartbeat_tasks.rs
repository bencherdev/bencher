use std::sync::Arc;

use dashmap::DashMap;
use tokio::task::AbortHandle;

use crate::model::runner::JobExecutionId;

/// Tracks spawned heartbeat timeout tasks, one per execution in flight, so they can be canceled
/// when a runner reconnects or an execution reaches a terminal state.
#[derive(Clone, Default)]
pub struct HeartbeatTasks {
    inner: Arc<DashMap<JobExecutionId, AbortHandle>>,
}

impl HeartbeatTasks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel any existing timeout for this execution, remove finished entries,
    /// and insert the new abort handle.
    pub fn insert(&self, execution_id: JobExecutionId, handle: AbortHandle) {
        // Cancel the previous timeout for this execution, if any.
        self.cancel(&execution_id);

        // Opportunistically clean up finished tasks to prevent unbounded growth.
        // DashMap is unordered (HashMap-based), so we cannot short-circuit the
        // retain scan. This is acceptable because the map is expected to be small
        // (one entry per execution in flight on this server).
        self.inner.retain(|_, h| !h.is_finished());

        self.inner.insert(execution_id, handle);
    }

    /// Cancel a specific execution's timeout.
    pub fn cancel(&self, execution_id: &JobExecutionId) {
        if let Some((_, handle)) = self.inner.remove(execution_id) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution_id(raw: i32) -> JobExecutionId {
        JobExecutionId::from_raw(raw)
    }

    /// Spawn a task that sleeps forever and register it in the tracker.
    fn spawn_sleeper(tasks: &HeartbeatTasks, id: JobExecutionId) -> AbortHandle {
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_hours(1)).await;
        });
        let abort = handle.abort_handle();
        tasks.insert(id, abort.clone());
        abort
    }

    #[tokio::test]
    async fn insert_cancels_previous_task_for_same_execution() {
        let tasks = HeartbeatTasks::new();
        let id = execution_id(1);

        let first_handle = spawn_sleeper(&tasks, id);
        // Insert a new task for the same execution: it aborts the first one.
        let _second_handle = spawn_sleeper(&tasks, id);

        // Give tokio a moment to propagate the abort.
        tokio::task::yield_now().await;
        assert!(
            first_handle.is_finished(),
            "First task should be aborted after re-insert"
        );
    }

    #[tokio::test]
    async fn cancel_aborts_task() {
        let tasks = HeartbeatTasks::new();
        let id = execution_id(2);

        let handle = spawn_sleeper(&tasks, id);
        assert!(
            !handle.is_finished(),
            "Task should be running before cancel"
        );

        tasks.cancel(&id);
        // Give tokio a moment to propagate the abort.
        tokio::task::yield_now().await;
        assert!(handle.is_finished(), "Task should be aborted after cancel");
    }

    #[tokio::test]
    async fn insert_cleans_up_finished_entries() {
        let tasks = HeartbeatTasks::new();

        // Insert a task that finishes immediately, then manually put its handle in the map.
        let finished_id = execution_id(10);
        let finished_handle = tokio::spawn(async {});
        let abort = finished_handle.abort_handle();
        finished_handle.await.expect("task should finish");
        tasks.inner.insert(finished_id, abort);

        // Insert a live task for a different execution, which triggers cleanup.
        let live_id = execution_id(11);
        let _live_handle = spawn_sleeper(&tasks, live_id);

        // The finished entry should have been cleaned up.
        assert!(
            !tasks.inner.contains_key(&finished_id),
            "Finished task entry should be cleaned up after insert"
        );
        assert!(
            tasks.inner.contains_key(&live_id),
            "Live task entry should remain"
        );
    }
}
