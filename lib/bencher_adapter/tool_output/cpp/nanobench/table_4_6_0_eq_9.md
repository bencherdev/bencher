Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |     mode |               ns/op |                op/s |    err% |     total | Limits
|------------:|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|         100 |        x |                8.60 |      116,265,465.20 |   10.4% |      0.01 | :wavy_dash: `with n` (Unstable with ~119,104.1 iters. Increase `minEpochIterations` to e.g. 1191041)
|abcdefghijklm|abcdefgh |                9.36 |      106,868,301.34 |   10.1% |      0.01 | :wavy_dash: `without n` (Unstable with ~120,555.9 iters. Increase `minEpochIterations` to e.g. 1205559)
