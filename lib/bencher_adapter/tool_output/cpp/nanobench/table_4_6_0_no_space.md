Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                8.43 |      118,691,785.85 |    4.1% |      0.01 | `plain`
|1,000,000,000|abcdefghi |                8.54 |      117,123,867.20 |    6.5% |      0.01 | :wavy_dash: `piped` (Unstable with ~94,674.3 iters. Increase `minEpochIterations` to e.g. 946743)
