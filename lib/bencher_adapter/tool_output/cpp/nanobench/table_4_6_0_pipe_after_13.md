Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| num_threads |     mode |               ns/op |                op/s |    err% |     total | Threads
|------------:|---------:|--------------------:|--------------------:|--------:|----------:|:--------
|           4 |    plain |               10.19 |       98,102,476.60 |   29.3% |      0.01 | :wavy_dash: `fill` (Unstable with ~84,612.1 iters. Increase `minEpochIterations` to e.g. 846121)
|           4 |      a|b |               15.88 |       62,967,653.25 |    2.5% |      0.01 | `fill`
