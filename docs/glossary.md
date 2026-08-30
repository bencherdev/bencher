# Glossary

Glossary of terms for Bencher concepts.

## Continuous Benchmarking 

- **Run**:
  A single continuous benchmarking invocation which generates a Report.
- **Report**:
  A collection of Benchmarks and their Metrics for a particular Branch and Testbed.
- **Series**:
  A distinct combination of Testbed, Benchmark, Parameter Set, and Measure.
  Each Series has its own Metric history and bills as its own Series.
  A Series does not include a Branch.
- **Line**:
  A Series plotted on one Branch.
- **Permutation**:
  A unique combination of Branch, Testbed, Benchmark, and Measure.
  A Permutation has a separate Line per Variant of its Benchmark.

## Dimensions

- **Branch**:
  The `git` ref used when running a Report (ie branch name or tag).
- **Head**:
  The current Head for a Branch.
  A Head may reference the most recent Start Point, if there is one.
  A Head is a sub-dimension of a Branch.
- **Start Point**:
  Another Branch at a specific version (and `git` hash, if available).
  Historical Metrics and optionally Thresholds are copied over from the Start Point.
- **Testbed**:
  The name of the testing environment used for a Run.
- **Spec**:
  A hardware specification that describes the resources used for a Run.
  When a Report is created, the Testbed's current Spec at that point in time
  is recorded with the Report.
  Only results from the same Testbed and Spec are used for Thresholds.
  When using Bencher Bare Metal, a Job declares the Spec it needs,
  and a Runner only claims Jobs for Specs it supports.
  A Spec is a sub-dimension of a Testbed.
- **Benchmark**:
  A named performance regression test.
  A Benchmark may be ignored so it does not generate Alerts.
- **Parameter Set**:
  A set of parameters used for a Benchmark.
  Each key maps to a JSON scalar: a string, a number, or a boolean.
  Values are canonicalized, so `16`, `16.0`, and `1.6e1` are the same value.
  A Parameter Set may carry at most 8 keys.
  A Parameter Set is a sub-dimension of a Benchmark.
- **Variant**:
  A Benchmark instantiated with one Parameter Set.
  If no Parameter Sets are specified, a Benchmark has a Singe Variant
  with an empty Parameter Set.
- **Measure**:
  The unit of measurement for a Metric.
  For example, `Latency` and `Throughput` with units of
  `nanoseconds (ns)` and `operations / second (ops/s)` respectively.
- **Metric**:
  A single, point-in-time performance regression test result:
  one named Value collected for a Measure.
  The point estimate is the `value` Metric.
  The former deprecated shape that collected up to three Values:
  `value`, `lower_value`, and `upper_value` is the Metric Triple.
  A Metric is sub-dimension of a Measure.

## Thresholds

- **Threshold**:
  Used to catch performance regressions.
  A Threshold is assigned to a unique combination of: Branch, Testbed, and Measure.
  An array of Parameter Sets can be specified to filter the Benchmarks a Threshold checks.
- **Model**:
  The combination of a Test and its parameters for a Threshold.
- **Test**:
  Used by a Threshold to detect performance regressions.
  For example, a z-score or t-test.
- **Boundary**:
  A Model must have a Lower Boundary, Upper Boundary, or both.
  A Lower Boundary is used when a smaller value would indicate a performance regression.
  An Upper Boundary is used when a larger value would indicate a performance regression.
- **Check**:
  A Threshold checks a Metric: its Model runs its Test against the Line history
  and calculates Boundary Limits.
- **Boundary Limit**:
  Each Boundary is used to calculate a Boundary Limit
  for each new Metric that a Threshold checks.
- **Alert**:
  An Alert is generated when a new Metric is
  below a Lower Boundary Limit or above an Upper Boundary Limit.
  An Alert is not generated when the Benchmark is ignored.

## Bare Metal

- **Runner**: 
  A remote benchmark executor that runs on dedicated, bare metal hardware.
  The runner opens a single WebSocket connection to the API server,
  claims Jobs that match its Specs, executes them, and reports back results.
- **Job**:
  Tracks the lifecycle of a remote benchmark execution request.
  A Job moves through: pending, claimed, and running then completed, failed, or canceled.
  A completed Job's results are processed into a Report.
- **Claim**:
  A Runner claims a pending Job.
  A Job declares the Spec it needs, and a Runner only claims Jobs for Specs it supports.
- **Image**:
  An OCI container image of benchmark code and its dependencies.
- **Sandbox**:
  An isolation mechanism that allows for safe multi-tenancy on a Runner.
  Sandboxed Jobs run in a Firecracker microVM.
  Non-sandboxed Jobs must be trusted and run directly on the host.
- **Host Tuning**:
  Changes made to the Runner's bare metal server to achieve consistent benchmark results.
