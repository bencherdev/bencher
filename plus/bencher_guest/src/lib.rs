#![cfg(feature = "plus")]

//! Bencher guest SDK for benchmark authors.
//!
//! This crate provides an optional SDK for benchmark code running inside a
//! Bencher Firecracker VM. It is **not** used by the runner itself — the runner
//! communicates with the guest via raw vsock (see `bencher_runner::firecracker::vsock`).
//!
//! Benchmark authors can use this crate to:
//! - Communicate with the host via vsock
//! - Send structured benchmark results back to the host
//! - Receive benchmark parameters from the host
//!
//! # Example
//!
//! ```ignore
//! use bencher_guest::{connect_to_host, send_results, BenchmarkResults};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to the host
//!     let mut conn = connect_to_host()?;
//!
//!     // Run your benchmark
//!     let start = std::time::Instant::now();
//!     // ... benchmark code ...
//!     let elapsed = start.elapsed();
//!
//!     // Send results back
//!     let results = BenchmarkResults::new()
//!         .with_metric("latency_ns", elapsed.as_nanos() as f64);
//!     send_results(&mut conn, &results)?;
//!
//!     Ok(())
//! }
//! ```

mod protocol;
mod vsock;

pub use protocol::{BenchmarkParams, BenchmarkResults, Metric};
pub use vsock::{
    DEFAULT_PORT, HOST_CID, VsockConnection, VsockError, connect_to_host, connect_to_host_port,
    send_results,
};
