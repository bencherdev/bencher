Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                7.86 |      127,230,245.47 |    0.5% |      0.01 | `plain`
|0,123,456,789 |abcdefghi |                7.31 |      136,758,207.35 |    1.6% |      0.01 | `piped`
