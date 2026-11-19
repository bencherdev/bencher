Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |               ns/op |                op/s |    err% |     total | Limits
|------------:|--------------------:|--------------------:|--------:|----------:|:-------
|1,000,000,000 |                8.51 |      117,528,321.64 |    2.3% |      0.01 | `with big n`
|                8.47 |      118,094,212.69 |    1.9% |      0.01 | `without n`
