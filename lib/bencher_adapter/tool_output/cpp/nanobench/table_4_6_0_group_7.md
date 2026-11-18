Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                8.35 |      119,800,312.77 |    1.5% |      0.01 | `plain`
|1,000,0000000 |abcdefghi |                7.86 |      127,186,669.90 |    2.4% |      0.01 | `piped`
