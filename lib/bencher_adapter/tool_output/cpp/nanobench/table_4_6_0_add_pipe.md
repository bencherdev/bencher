Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                7.10 |      140,823,654.85 |    0.6% |      0.01 | `plain`
|1234567890123|123456789 |                7.06 |      141,560,570.32 |    0.4% |      0.01 | `piped`
