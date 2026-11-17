# Thresholds and Alerts

Thresholds define statistical boundaries for benchmark results.
When a new result exceeds the boundary, Bencher generates an alert,
signaling a potential performance regression.

## Inline Threshold Creation

The simplest way to configure thresholds is inline with `bencher run`:
```bash
bencher run \
  --adapter rust_criterion \
  --threshold-measure latency \
  --threshold-metric typical \
  --threshold-test t_test \
  --threshold-upper-boundary 0.99 \
  "cargo bench"
```

This creates (or updates) a threshold for the `typical` metric of the `latency`
measure using a Student's t-test with a 99% confidence upper boundary.

## Choosing the Metric

At BMF version 1, where every new project starts, a harness adapter writes each
measure with the named metrics its harness reports (`median`, `mean`, `p99`, ...),
and a threshold checks one of them. Name it with `--threshold-metric`: for the
adapter's main measure, start with its **threshold first** metric from the adapter
table in [local-runs.md](./local-runs.md#adapters); for another measure, the
adapter's section on the [adapters page](https://bencher.dev/docs/explanation/adapters/)
names the metric to start with. That metric is also the one the PR comment and the
run output show while no threshold checks the measure.
`bencher mock`, `--build-time`, `--file-size`, and BMF JSON that writes `value`
take `--threshold-metric value`.

At version 1, a threshold flag group without `--threshold-metric` creates no
threshold: the run lands, and the run output and the PR comment show a
skipped-threshold warning with a hint that names `--threshold-metric`. Once any
group names a metric or parameters, every group needs a metric.

At BMF version 0, `bencher run`'s threshold flags check `value`, and the CLI
refuses `--threshold-metric` and `--threshold-parameters` before it runs the
benchmarks or sends the report.
To move a version 0 project to version 1, see
[local-runs.md](./local-runs.md#bmf-version).

## Choosing the Variants

At version 1, a benchmark has a variant for each set of parameters it runs with,
and a threshold checks every variant unless `--threshold-parameters` filters them.
The filter is a JSON object that matches every variant whose parameters hold the
same value for each key it names:
```bash
bencher run \
  --adapter python_pytest \
  --file results.json \
  --threshold-measure latency \
  --threshold-metric median \
  --threshold-parameters '{"size": 64}' \
  --threshold-test t_test \
  --threshold-upper-boundary 0.99 \
  "pytest --benchmark-json results.json"
```

## Fail CI on Alert

Add `--error-on-alert` (alias `--err`) to exit non-zero when an alert fires:
```bash
bencher run \
  --adapter rust_criterion \
  --threshold-measure latency \
  --threshold-metric typical \
  --threshold-test t_test \
  --threshold-upper-boundary 0.99 \
  --error-on-alert \
  "cargo bench"
```

## Statistical Models

| Model | Slug | Best For | Key Params |
|-------|------|----------|------------|
| Static | `static` | Fixed-value thresholds | `lower_boundary`, `upper_boundary` (absolute values) |
| Percentage | `percentage` | Percentage change from mean | `upper_boundary` (e.g., 0.10 = 10% regression) |
| z-score | `z_score` (alias: `z`) | Normal distribution, large samples (>30) | `upper_boundary` (cumulative percentage, e.g., 0.99) |
| Student's t-test | `t_test` (alias: `t`) | Normal distribution, small samples (<30) | `upper_boundary` (cumulative percentage, e.g., 0.99) |
| Log Normal | `log_normal` | Log-normal distributions (common in latency) | `upper_boundary` (cumulative percentage) |
| Interquartile Range | `iqr` | Skewed data, outlier-robust | `upper_boundary` (IQR multiplier, e.g., 3.0) |
| Delta IQR | `delta_iqr` | Change-based IQR | `upper_boundary` (delta IQR multiplier) |

## Choosing a Model

- **Latency benchmarks:** Use `t_test` (small sample) or `log_normal` (right-skewed data)
- **Throughput benchmarks:** Use `t_test` or `z_score` (often normally distributed)
- **Known fixed limits:** Use `static` with exact boundary values
- **Noisy environments:** Use `iqr` or `delta_iqr` (outlier-robust)
- **Percentage-based checks:** Use `percentage` (e.g., "alert if >10% slower")

## Boundary Configuration

Thresholds can have a lower boundary, upper boundary, or both:

```bash
# Alert if latency increases (upper only, most common)
--threshold-measure latency --threshold-metric typical --threshold-test t --threshold-upper-boundary 0.99

# Alert if throughput decreases (lower only)
--threshold-measure throughput --threshold-metric typical --threshold-test t --threshold-lower-boundary 0.99

# Alert on any change (both)
--threshold-measure latency --threshold-metric typical --threshold-test t_test \
  --threshold-lower-boundary 0.95 --threshold-upper-boundary 0.95
```

## Sample Size, Window, and Reset

A threshold checks each new metric against the history of the same branch,
testbed, variant, measure, and metric.

Control the historical data used:
```bash
bencher run \
  --adapter rust_criterion \
  --threshold-measure latency \
  --threshold-metric typical \
  --threshold-test t_test \
  --threshold-min-sample-size 2 \
  --threshold-max-sample-size 64 \
  --threshold-window 2592000 \
  --threshold-upper-boundary 0.99 \
  "cargo bench"
```

| Flag | Purpose |
|------|---------|
| `--threshold-min-sample-size <n>` | Minimum historical samples required |
| `--threshold-max-sample-size <n>` | Maximum historical samples to consider |
| `--threshold-window <secs>` | Time window in seconds for historical data |
| `--thresholds-reset` | Reset all unspecified thresholds for the branch and testbed |

Use `_` as a value to leave an option unset for one threshold group
(see [Multiple Measures and Metrics](#multiple-measures-and-metrics)).

The `--start-point-max-versions` flag controls how much history is available
(default: 64 versions). More history gives better statistical confidence
but may include stale data from before an intentional performance change.

## Managing Thresholds via CLI

```bash
# List thresholds for a project
bencher threshold list my-project

# View a specific threshold
bencher threshold view my-project <threshold-uuid>

# Create a threshold outside of bencher run
bencher threshold create my-project \
  --branch main --testbed localhost \
  --measure latency --metric median --parameters '{"size": 64}' \
  --test t --upper-boundary 0.99

# Delete a threshold
bencher threshold delete my-project <threshold-uuid>
```

Without `--metric`, `bencher threshold create` checks `value`, and without
`--parameters`, every variant. At BMF version 0, leave out `--metric` and
`--parameters`: a version 0 result has no parameters, and its metrics are `value`
with an optional `lower_value` and `upper_value`, so a threshold with a parameters
filter, or on any other metric, never checks anything.
The metric and the parameters filter are part of what a threshold is:
`bencher threshold update` changes only its model, and another metric or filter
is another threshold.

## Multiple Measures and Metrics

If your benchmark produces multiple measures (e.g., latency and throughput),
or you want to check more than one metric of a measure,
add a threshold flag group for each.
The CLI pairs the nth value of each threshold flag with the nth `--threshold-measure`,
so once a group gives an option, every group gives it.
Every group names its measure, metric, and test;
for `--threshold-parameters`, the sample sizes, the window, or a boundary
that a group goes without, give `_`:
```bash
bencher run \
  --adapter rust_criterion \
  --threshold-measure latency \
  --threshold-metric typical \
  --threshold-test t_test \
  --threshold-lower-boundary _ \
  --threshold-upper-boundary 0.99 \
  --threshold-measure throughput \
  --threshold-metric typical \
  --threshold-test t_test \
  --threshold-lower-boundary 0.95 \
  --threshold-upper-boundary _ \
  "cargo bench"
```
