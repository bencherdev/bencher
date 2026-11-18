Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                7.15 |      139,812,257.78 |    0.8% |      0.01 | `plain`
|1,000,000,000 |abcdefgh |                7.81 |      127,981,194.04 |    2.6% |      0.01 | `piped`
