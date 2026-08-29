# Glossary

Working definitions for Bencher's dimensions and units. The canonical prose for
public concepts is the documentation under
[Benchmarking Concepts](https://bencher.dev/docs/explanation/benchmarking/);
entries marked with its anchor quote it. Entries marked *(proposed)* cover
concepts the benchmark parameters work introduces and are open for refinement.

## Dimensions

- **Branch**: The `git` ref used when running a Report (ie branch name or tag).
- **Head**: The most recent instance of the Branch. It references the most
  recent Start Point, if there is one.
- **Start Point**: Another Branch at a specific version (and `git` hash, if
  available). Historical Metrics and optionally Thresholds are copied over from
  the Start Point.
- **Testbed**: The name of the testing environment used when running a Report.
- **Spec**: A hardware specification that describes the resources available to
  a Runner. When a Report is created, the Testbed's current Spec at that point
  in time is recorded with the Report. Only results from the same Testbed and
  Spec are used for Thresholds.
- **Benchmark**: A named performance regression test. If the performance
  regression test is new to Bencher, then a Benchmark is automatically created.
- **Parameter Set** *(proposed)*: A set of parameters reported for a Benchmark.
  Each key maps to a JSON scalar: a string, a number, or a boolean. Values are
  canonicalized, so `16`, `16.0`, and `1.6e1` are the same value. A Parameter
  Set may carry at most 8 keys. Every Benchmark starts with the empty
  Parameter Set.
- **Variant** *(proposed)*: A Benchmark with one concrete Parameter Set. A
  Benchmark that never reports parameters has a single Variant, the empty
  Parameter Set.
- **Measure**: The unit of measurement for a Metric. By default all Projects
  start with a `Latency` and `Throughput` Measure with units of
  `nanoseconds (ns)` and `operations / second (ops/s)` respectively.

## Units

- **Report**: A collection of Benchmarks and their Metrics for a particular
  Branch and Testbed.
- **Metric** *(proposed revision)*: A single, point-in-time performance
  regression test result: one named Value collected for a Measure. The point
  estimate is the `value` Metric. The former shape that collected up to three
  Values (`value`, `lower_value`, and `upper_value`) is the Metric triple, and
  the deprecated fields that carry it are rebuilt from those three Metrics.
- **Series** *(proposed)*: A distinct combination of Testbed, Benchmark,
  Parameter Set, and Measure. Each Series has its own Metric history and bills
  as its own Series. A Series does not include a Branch.
- **Line** *(proposed)*: One Series plotted on one Branch. A Perf query returns
  at most the first 256 Lines.
- **Permutation** *(proposed)*: One combination of Branch, Testbed, Benchmark,
  and Measure that a Perf query runs. Each Permutation fans out into one Line
  per Variant of its Benchmark.

## Thresholds

- **Threshold**: Used to catch performance regressions. A Threshold is assigned
  to a unique combination of: Branch, Testbed, and Measure.
- **Test**: Used by a Threshold to detect performance regressions.
- **Model**: The combination of a Test and its parameters for a Threshold.
- **Boundary**: A Model must have a Lower Boundary, Upper Boundary, or both. A
  Lower Boundary is used when a smaller value would indicate a performance
  regression. An Upper Boundary is used when a larger value would indicate a
  performance regression.
- **Boundary Limit**: Each Boundary is used to calculate a Boundary Limit for a
  new Metric.
- **Check** *(proposed)*: A Threshold checks a Metric: its Model runs its Test
  against the Series history and calculates Boundary Limits. Gating is the CI
  decision an Alert feeds, such as `bencher run --err`; Thresholds check,
  pipelines gate.
- **Alert**: Generated when a new Metric fails a Test by being below a Lower
  Boundary Limit or above an Upper Boundary Limit. An Alert is not generated
  when the Benchmark is ignored.
