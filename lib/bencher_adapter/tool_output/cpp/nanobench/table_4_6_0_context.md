Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     size |               ns/op |                op/s |    err% |     total | Context
|---------:|--------------------:|--------------------:|--------:|----------:|:--------
|       10 |                6.93 |      144,218,982.10 |    1.2% |      0.01 | `vector fill`
|      100 |               16.60 |       60,257,343.25 |    0.2% |      0.01 | `vector fill`
