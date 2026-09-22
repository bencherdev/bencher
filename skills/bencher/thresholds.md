# Thresholds and Alerts

Thresholds define statistical boundaries for benchmark results.
When a new result exceeds the boundary, Bencher generates an alert,
signaling a potential performance regression.

## Inline Threshold Creation

The simplest way to configure thresholds is inline with `bencher run`:
```bash
bencher run \
  --threshold-measure latency \
  --threshold-test t_test \
  --threshold-upper-boundary 0.99 \
  "cargo bench"
```

This creates (or updates) a threshold for the `latency` measure using a
Student's t-test with a 99% confidence upper boundary.

## Fail CI on Alert

Add `--error-on-alert` (alias `--err`) to exit non-zero when an alert fires:
```bash
bencher run \
  --threshold-measure latency \
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
--threshold-measure latency --threshold-test t --threshold-upper-boundary 0.99

# Alert if throughput decreases (lower only)
--threshold-measure throughput --threshold-test t --threshold-lower-boundary 0.99

# Alert on any change (both)
--threshold-measure latency --threshold-test t_test \
  --threshold-lower-boundary 0.95 --threshold-upper-boundary 0.95
```

## Sample Size, Window, and Reset

Thresholds use historical data from the same branch, testbed, and measure.

Control the historical data used:
```bash
bencher run \
  --threshold-measure latency \
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

Use `_` as a value to explicitly ignore a parameter (leave it unset).

The `--start-point-max-versions` flag controls how much history is available
(default: 255 versions). More history gives better statistical confidence
but may include stale data from before an intentional performance change.

## Managing Thresholds via CLI

```bash
# List thresholds for a project
bencher threshold list my-project

# View a specific threshold
bencher threshold view my-project <threshold-uuid>

# Create a threshold outside of bencher run
bencher threshold create my-project \
  --branch main --testbed localhost --measure latency \
  --test t --upper-boundary 0.99

# Delete a threshold
bencher threshold delete my-project <threshold-uuid>
```

## Multiple Measures

If your benchmark produces multiple measures (e.g., latency and throughput),
add multiple threshold flag groups:
```bash
bencher run \
  --threshold-measure latency \
  --threshold-test t_test \
  --threshold-upper-boundary 0.99 \
  --threshold-measure throughput \
  --threshold-test t_test \
  --threshold-lower-boundary 0.95 \
  "cargo bench"
```
