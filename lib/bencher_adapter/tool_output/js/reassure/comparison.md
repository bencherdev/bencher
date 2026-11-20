# Performance Comparison Report

- **Current**: (unknown) - 2026-11-15 05:13:15Z
- **Baseline**: (unknown) - 2026-11-15 05:13:12Z

### Significant Changes To Duration

<details open>
<summary>Show entries</summary>

| Name           | Type           | Duration                               | Count  |
| -------------- | -------------- | -------------------------------------- | ------ |
| async function | async function | 0.0 ms → 0.0 ms (+0.0 ms, +40.6%) 🔴🔴 | 1 → 1  |

</details>

<details>
<summary>Show details</summary>

| Name           | Type           | Duration                                                                                                                                                                                                                                                                                                                          | Count                                                                                                                                     |
| -------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| async function | async function | **Baseline**<br/>Mean: 0.0 ms<br/>Stdev: 0.0 ms (15.5%)<br/>Runs: 0.0 0.0 0.0 0.0 0.0<br/>Warmup runs: 0.0<br/>Removed outliers: (none)<br/>Stability: 15.5%<br/><br/>**Current**<br/>Mean: 0.0 ms<br/>Stdev: 0.0 ms (20.3%)<br/>Runs: 0.0 0.0 0.0 0.0 0.0<br/>Warmup runs: 0.0<br/>Removed outliers: (none)<br/>Stability: 20.3% | **Baseline**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1<br/><br/>**Current**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1 |

</details>

### Meaningless Changes To Duration

<details>
<summary>Show entries</summary>

| Name               | Type     | Duration                           | Count  |
| ------------------ | -------- | ---------------------------------- | ------ |
| fib function       | function | 0.2 ms → 0.2 ms (-0.0 ms, -11.6%)  | 1 → 1  |
| List 100           | render   | 2.3 ms → 2.3 ms (+0.0 ms, +1.4%)   | 1 → 1  |
| List with scenario | render   | 1.8 ms → 1.8 ms (-0.0 ms, -1.8%)   | 3 → 3  |

</details>

<details>
<summary>Show details</summary>

| Name               | Type     | Duration                                                                                                                                                                                                                                                                                                                                                                | Count                                                                                                                                                                                               |
| ------------------ | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| fib function       | function | **Baseline**<br/>Mean: 0.2 ms<br/>Stdev: 0.1 ms (28.8%)<br/>Runs: 0.3 0.2 0.2 0.2 0.2 0.2 0.2 0.1 0.1 0.1<br/>Warmup runs: 0.4<br/>Removed outliers: (none)<br/>Stability: 28.8%<br/><br/>**Current**<br/>Mean: 0.2 ms<br/>Stdev: 0.0 ms (5.6%)<br/>Runs: 0.2 0.2 0.1 0.2 0.2 0.2 0.2 0.2 0.1 0.2<br/>Warmup runs: 0.2<br/>Removed outliers: (none)<br/>Stability: 5.6% | **Baseline**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1 1 1 1 1 1<br/><br/>**Current**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1 1 1 1 1 1                                       |
| List 100           | render   | **Baseline**<br/>Mean: 2.3 ms<br/>Stdev: 0.4 ms (18.6%)<br/>Runs: 2.2 1.9 2.1 3.3 2.0 2.3 2.1 2.0 2.1 2.7<br/>Warmup runs: 6.0<br/>Removed outliers: (none)<br/>Stability: 18.6%<br/><br/>**Current**<br/>Mean: 2.3 ms<br/>Stdev: 0.2 ms (8.5%)<br/>Runs: 2.2 2.1 2.4 2.6 2.4 2.2 2.6 2.0 2.1 2.2<br/>Warmup runs: 6.1<br/>Removed outliers: (none)<br/>Stability: 8.5% | **Baseline**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1 1 1 1 1 1<br/>Render issues:<br/><br/>**Current**<br/>Mean: 1<br/>Stdev: 0 (0.0%)<br/>Runs: 1 1 1 1 1 1 1 1 1 1<br/>Render issues: |
| List with scenario | render   | **Baseline**<br/>Mean: 1.8 ms<br/>Stdev: 0.2 ms (9.7%)<br/>Runs: 1.8 2.0 1.9 1.8 1.6<br/>Warmup runs: 2.4<br/>Removed outliers: (none)<br/>Stability: 9.7%<br/><br/>**Current**<br/>Mean: 1.8 ms<br/>Stdev: 0.2 ms (8.6%)<br/>Runs: 1.8 2.0 1.6 1.8 1.7<br/>Warmup runs: 1.9<br/>Removed outliers: (none)<br/>Stability: 8.6%                                           | **Baseline**<br/>Mean: 3<br/>Stdev: 0 (0.0%)<br/>Runs: 3 3 3 3 3<br/>Render issues:<br/><br/>**Current**<br/>Mean: 3<br/>Stdev: 0 (0.0%)<br/>Runs: 3 3 3 3 3<br/>Render issues:                     |

</details>

### Render Count Changes

*There are no entries*

### Render Issues

*There are no entries*

### Added Entries

*There are no entries*

### Removed Entries

*There are no entries*

### Stability

| Name             | Baseline | Current |
| ---------------- | -------- | ------- |
| Weighted Average | 15.2%    | 8.5%    |