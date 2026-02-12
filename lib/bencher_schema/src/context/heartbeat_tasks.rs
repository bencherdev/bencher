use std::sync::Arc;

use dashmap::DashMap;
use tokio::task::AbortHandle;

use crate::model::runner::JobId;

/// Tracks spawned heartbeat timeout tasks so they can be canceled
/// when a runner reconnects or a job reaches a terminal state.
#[derive(Clone, Default)]
pub struct HeartbeatTasks {
    inner: Arc<DashMap<JobId, AbortHandle>>,
}

impl HeartbeatTasks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel any existing timeout for this job, remove finished entries,
    /// and insert the new abort handle.
    pub fn insert(&self, job_id: JobId, handle: AbortHandle) {
        // Cancel the previous timeout for this job, if any.
        self.cancel(&job_id);

        // Opportunistically clean up finished tasks to prevent unbounded growth.
        self.inner.retain(|_, h| !h.is_finished());

        self.inner.insert(job_id, handle);
    }

    /// Cancel a specific job's timeout (e.g., when the job reaches a terminal state).
    pub fn cancel(&self, job_id: &JobId) {
        if let Some((_, handle)) = self.inner.remove(job_id) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_id(raw: i32) -> JobId {
        JobId::from_raw(raw)
    }

    /// Spawn a task that sleeps forever and register it in the tracker.
    fn spawn_sleeper(tasks: &HeartbeatTasks, id: JobId) -> AbortHandle {
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        });
        let abort = handle.abort_handle();
        tasks.insert(id, abort.clone());
        abort
    }

    #[tokio::test]
    async fn insert_cancels_previous_task_for_same_job() {
        let tasks = HeartbeatTasks::new();
        let id = job_id(1);

        let first_handle = spawn_sleeper(&tasks, id);
        // Insert a new task for the same job — should abort the first one.
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
        let id = job_id(2);

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
        let finished_id = job_id(10);
        let finished_handle = tokio::spawn(async {});
        let abort = finished_handle.abort_handle();
        finished_handle.await.expect("task should finish");
        tasks.inner.insert(finished_id, abort);

        // Insert a live task for a different job — triggers cleanup.
        let live_id = job_id(11);
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
