#![cfg(feature = "plus")]

//! Bencher guest library.
//!
//! This crate provides utilities for benchmark code running inside a Bencher VM.
//! It handles:
//! - Communication with the host via vsock
//! - Sending benchmark results back to the host
//! - Receiving benchmark parameters from the host
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
pub use vsock::{connect_to_host, VsockConnection};
