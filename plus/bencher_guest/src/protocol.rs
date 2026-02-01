//! Benchmark communication protocol.
//!
//! This defines the JSON schema for communication between the host and guest.

use serde::{Deserialize, Serialize};

/// Parameters sent from the host to the guest before a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkParams {
    /// Unique identifier for this benchmark run.
    pub run_id: String,

    /// The benchmark command to execute (if not using the default entrypoint).
    #[serde(default)]
    pub command: Option<String>,

    /// Environment variables to set.
    #[serde(default)]
    pub env: Vec<(String, String)>,

    /// Additional configuration as a JSON object.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Results sent from the guest to the host after a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Whether the benchmark completed successfully.
    pub success: bool,

    /// Error message if the benchmark failed.
    #[serde(default)]
    pub error: Option<String>,

    /// Collected metrics.
    #[serde(default)]
    pub metrics: Vec<Metric>,

    /// Raw output from the benchmark (stdout).
    #[serde(default)]
    pub stdout: String,

    /// Raw error output (stderr).
    #[serde(default)]
    pub stderr: String,

    /// Duration of the benchmark run in nanoseconds.
    #[serde(default)]
    pub duration_ns: u64,
}

/// A single benchmark metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// Name of the metric (e.g., `latency_ns`, `throughput_ops_sec`).
    pub name: String,

    /// Value of the metric.
    pub value: f64,

    /// Unit of the metric (e.g., `ns`, `ops/sec`, `bytes`).
    #[serde(default)]
    pub unit: Option<String>,
}

impl BenchmarkResults {
    /// Create a new successful results object.
    #[must_use]
    pub fn new() -> Self {
        Self {
            success: true,
            error: None,
            metrics: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            duration_ns: 0,
        }
    }

    /// Create a new failed results object.
    #[must_use]
    pub fn failure<S: Into<String>>(error: S) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            metrics: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            duration_ns: 0,
        }
    }

    /// Add a metric to the results.
    #[must_use]
    pub fn with_metric<S: Into<String>>(mut self, name: S, value: f64) -> Self {
        self.metrics.push(Metric {
            name: name.into(),
            value,
            unit: None,
        });
        self
    }

    /// Add a metric with a unit to the results.
    #[must_use]
    pub fn with_metric_unit<S: Into<String>, U: Into<String>>(
        mut self,
        name: S,
        value: f64,
        unit: U,
    ) -> Self {
        self.metrics.push(Metric {
            name: name.into(),
            value,
            unit: Some(unit.into()),
        });
        self
    }

    /// Set the stdout output.
    #[must_use]
    pub fn with_stdout<S: Into<String>>(mut self, stdout: S) -> Self {
        self.stdout = stdout.into();
        self
    }

    /// Set the stderr output.
    #[must_use]
    pub fn with_stderr<S: Into<String>>(mut self, stderr: S) -> Self {
        self.stderr = stderr.into();
        self
    }

    /// Set the duration in nanoseconds.
    #[must_use]
    pub fn with_duration_ns(mut self, duration_ns: u64) -> Self {
        self.duration_ns = duration_ns;
        self
    }
}

impl Default for BenchmarkResults {
    fn default() -> Self {
        Self::new()
    }
}

impl Metric {
    /// Create a new metric.
    #[must_use]
    pub fn new<S: Into<String>>(name: S, value: f64) -> Self {
        Self {
            name: name.into(),
            value,
            unit: None,
        }
    }

    /// Create a new metric with a unit.
    #[must_use]
    pub fn with_unit<S: Into<String>, U: Into<String>>(name: S, value: f64, unit: U) -> Self {
        Self {
            name: name.into(),
            value,
            unit: Some(unit.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_results_builder() {
        let results = BenchmarkResults::new()
            .with_metric("latency_ns", 1000.0)
            .with_metric_unit("throughput", 500.0, "ops/sec")
            .with_duration_ns(1_000_000);

        assert!(results.success);
        assert_eq!(results.metrics.len(), 2);
        assert_eq!(results.duration_ns, 1_000_000);
    }

    #[test]
    fn test_results_serialization() {
        let results = BenchmarkResults::new()
            .with_metric("test", 42.0);

        let json = serde_json::to_string(&results).expect("Failed to serialize");
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"test\""));
    }
}
