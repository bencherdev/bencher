Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |     size |               ns/op |                op/s |    err% |     total | Limits
|------------:|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|         100 |        1 |                8.45 |      118,282,477.64 |    1.6% |      0.01 | `with n`
|123456789012 |                8.26 |      121,028,966.46 |    0.9% |      0.01 | `without n`
