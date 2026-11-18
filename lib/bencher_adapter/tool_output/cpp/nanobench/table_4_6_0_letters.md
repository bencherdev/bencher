Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|     mode |               ns/op |                op/s |    err% |     total | Limits
|---------:|--------------------:|--------------------:|--------:|----------:|:-------
|        x |                7.74 |      129,213,061.19 |    1.8% |      0.01 | `plain`
|a,bcd,efg,hij |abcdefghi |                7.41 |      134,863,841.61 |    1.9% |      0.01 | `piped`
