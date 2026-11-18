Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |               ns/op |                op/s |    err% |     total | Limits
|------------:|--------------------:|--------------------:|--------:|----------:|:-------
|         100 |                7.28 |      137,343,756.25 |    2.3% |      0.01 | `with n`
|           0 |                7.38 |      135,524,827.46 |    0.4% |      0.01 | `with n below 1`
