Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|               ns/op |                op/s |    err% |     total | Limits
|--------------------:|--------------------:|--------:|----------:|:-------
|                8.33 |      120,050,254.22 |    1.4% |      0.01 | `without n`
|1,000,000,000 |                8.23 |      121,502,989.34 |    0.3% |      0.01 | `with big n`
