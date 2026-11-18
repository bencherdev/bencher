Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                8.22 |      121,599,474.64 |    0.3% |      0.01 | `plain`
|1234,567,890 |abcdefghi |                8.50 |      117,699,528.28 |    4.6% |      0.01 | `piped`
