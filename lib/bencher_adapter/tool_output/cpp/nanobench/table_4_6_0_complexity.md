Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |               ns/op |                op/s |    err% |     total | Sizes
|------------:|--------------------:|--------------------:|--------:|----------:|:------
|         100 |                9.15 |      109,347,220.51 |    0.3% |      0.01 | `with n`
|                9.32 |      107,267,132.27 |    1.1% |      0.01 | `without n`
