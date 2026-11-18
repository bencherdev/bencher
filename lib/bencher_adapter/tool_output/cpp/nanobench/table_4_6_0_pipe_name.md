Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|      a|b |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        7 |                8.24 |      121,376,031.49 |    0.4% |      0.01 | `fill`
|        8 |                8.24 |      121,395,202.14 |    0.2% |      0.01 | `fill`
