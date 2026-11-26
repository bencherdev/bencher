//! Memory for upload part buffers, one pool for every request.
//!
//! A request takes a part's worth from the pool before it fills a part, and
//! the part gives it back once it is stored or has failed. A request that finds
//! too little free waits, so its client slows: no upload fails for want of memory.
//! A part that holds its buffer must fill within [`FILL_DEADLINE`], or its request
//! fails and gives the buffer back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::metrics;
use crate::storage::OciStorageError;

/// The pool counts in KiB, so that a part of the largest size still fits in one take.
const KIB: u64 = 1024;

/// Longest a part may hold its buffer while it fills from the request body, so a
/// stalled client cannot keep the pool from the requests that wait for it.
pub(crate) const FILL_DEADLINE: Duration = Duration::from_mins(5);

pub(crate) struct BufferPool {
    semaphore: Arc<Semaphore>,
    /// The pool's size in KiB
    capacity: u32,
}

/// A part's share of the pool, given back when dropped.
pub(crate) struct Buffer {
    _permit: OwnedSemaphorePermit,
    /// When the part must be full: [`FILL_DEADLINE`] after it took the buffer,
    /// or after its committed tail was read back into it
    fill_by: tokio::time::Instant,
}

impl BufferPool {
    /// A pool of `max_upload_memory` bytes, raised to one part of `part_size` bytes.
    pub(crate) fn new(max_upload_memory: u64, part_size: u64) -> Self {
        let capacity = kib(max_upload_memory.max(part_size));
        Self {
            semaphore: Arc::new(Semaphore::new(capacity as usize)),
            capacity,
        }
    }

    /// Takes a buffer for a part of `part_size` bytes, and waits while the pool
    /// has too little free. A part larger than the whole pool takes all of it.
    pub(crate) async fn take(&self, part_size: u64) -> Result<Buffer, OciStorageError> {
        let start = Instant::now();
        let permit = Arc::clone(&self.semaphore)
            .acquire_many_owned(kib(part_size).min(self.capacity))
            .await
            .map_err(|_closed| {
                OciStorageError::Storage("The upload buffer pool is closed".to_owned())
            })?;
        metrics::buffer_taken(start.elapsed());
        Ok(Buffer {
            _permit: permit,
            fill_by: tokio::time::Instant::now() + FILL_DEADLINE,
        })
    }
}

impl Buffer {
    pub(crate) fn fill_by(&self) -> tokio::time::Instant {
        self.fill_by
    }

    /// Starts the fill deadline again, for a part whose first bytes came from
    /// storage and not from the request body.
    pub(crate) fn start_fill(&mut self) {
        self.fill_by = tokio::time::Instant::now() + FILL_DEADLINE;
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        metrics::buffer_released();
    }
}

fn kib(bytes: u64) -> u32 {
    u32::try_from(bytes.div_ceil(KIB)).unwrap_or(u32::MAX)
}
