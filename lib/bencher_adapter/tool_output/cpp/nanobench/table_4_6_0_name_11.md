Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN | threadcount |               ns/op |                op/s |    err% |     total | Limits
|------------:|------------:|--------------------:|--------------------:|--------:|----------:|:-------
|         100 |           4 |                8.49 |      117,795,013.09 |    1.1% |      0.01 | `with n`
|           4 |                8.46 |      118,241,138.66 |    1.1% |      0.01 | `without n`
