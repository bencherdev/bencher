Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| complexityN |     mode |               ns/op |                op/s |    err% |     total | Limits
|------------:|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|         100 |        x |                6.99 |      143,114,224.42 |    1.3% |      0.01 | `with n`
|abcdefghijklm|abcdefghi |                7.18 |      139,282,507.23 |    0.3% |      0.01 | `without n`
