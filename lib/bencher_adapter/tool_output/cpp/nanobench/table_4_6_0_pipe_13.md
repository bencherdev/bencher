Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|abcdefghijklm|x |               16.47 |       60,701,595.82 |    5.5% |      0.01 | :wavy_dash: `fill` (Unstable with ~67,202.9 iters. Increase `minEpochIterations` to e.g. 672029)
|1234567890123|y |                8.36 |      119,562,622.03 |    1.2% |      0.01 | `fill`
