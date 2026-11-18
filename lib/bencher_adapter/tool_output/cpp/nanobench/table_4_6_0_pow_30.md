Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|               ns/op |                op/s |    err% |     total | Limits
|--------------------:|--------------------:|--------:|----------:|:-------
|                7.28 |      137,363,858.83 |    1.0% |      0.01 | `without n`
|1,073,741,824 |                7.31 |      136,830,989.41 |    1.0% |      0.01 | `with 2^30`
