use std::{
    io::{Read as _, Write as _},
    time::{Duration, Instant},
};

use super::BenchmarkResult;

const IO_BUFFER_SIZE: usize = 4096;

/// I/O jitter benchmark: repeatedly write and read a small temp file with fsync.
/// Sensitive to scheduler latency, I/O virtualization overhead, and shared storage contention.
pub fn run_io_benchmark(duration: Duration) -> Result<BenchmarkResult, IoError> {
    let dir = tempfile::tempdir().map_err(IoError::CreateTempDir)?;
    let file_path = dir.path().join("noise_io_test");
    let buf: Vec<u8> = vec![0xAB; IO_BUFFER_SIZE];

    let mut samples = Vec::new();
    let start = Instant::now();

    while start.elapsed() < duration {
        let iter_start = Instant::now();

        // Write
        {
            let mut file = std::fs::File::create(&file_path).map_err(IoError::CreateFile)?;
            file.write_all(&buf).map_err(IoError::WriteFile)?;
            file.sync_all().map_err(IoError::SyncFile)?;
        }

        // Read
        {
            let mut file = std::fs::File::open(&file_path).map_err(IoError::OpenFile)?;
            let mut read_buf = vec![0u8; IO_BUFFER_SIZE];
            file.read_exact(&mut read_buf).map_err(IoError::ReadFile)?;
        }

        let elapsed = iter_start.elapsed();
        samples.push(elapsed.as_secs_f64() * 1e9);
    }

    Ok(BenchmarkResult::from_samples(samples))
}

#[derive(thiserror::Error, Debug)]
pub enum IoError {
    #[error("Failed to create temp directory: {0}")]
    CreateTempDir(std::io::Error),
    #[error("Failed to create temp file: {0}")]
    CreateFile(std::io::Error),
    #[error("Failed to write to temp file: {0}")]
    WriteFile(std::io::Error),
    #[error("Failed to sync temp file: {0}")]
    SyncFile(std::io::Error),
    #[error("Failed to open temp file: {0}")]
    OpenFile(std::io::Error),
    #[error("Failed to read temp file: {0}")]
    ReadFile(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_benchmark_produces_samples() {
        let result = run_io_benchmark(Duration::from_millis(200)).unwrap();
        assert!(result.mean_ns > 0.0);
        assert!(result.iterations > 0);
    }
}
