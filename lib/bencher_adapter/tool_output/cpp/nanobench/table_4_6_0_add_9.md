Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |               10.96 |       91,258,815.06 |   16.2% |      0.01 | :wavy_dash: `plain` (Unstable with ~109,533.4 iters. Increase `minEpochIterations` to e.g. 1095334)
|abcdefghijklm|abcdefgh |               11.52 |       86,808,458.94 |   20.6% |      0.01 | :wavy_dash: `piped` (Unstable with ~83,636.5 iters. Increase `minEpochIterations` to e.g. 836365)
