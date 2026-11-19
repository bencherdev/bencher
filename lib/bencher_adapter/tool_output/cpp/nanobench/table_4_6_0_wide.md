Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


|       c0 |       c1 |       c2 |       c3 |       c4 |       c5 |       c6 |       c7 |       c8 |       c9 |      c10 |      c11 |      c12 |      c13 |      c14 |      c15 |               ns/op |                op/s |    err% |     total | Wide
|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|---------:|--------------------:|--------------------:|--------:|----------:|:-----
|        0 |        1 |        2 |        3 |        4 |        5 |        6 |        7 |        8 |        9 |       10 |       11 |       12 |       13 |       14 |          |                9.06 |      110,400,090.91 |    0.9% |      0.01 | `fifteen`
|        0 |        1 |        2 |        3 |        4 |        5 |        6 |        7 |        8 |        9 |       10 |       11 |       12 |       13 |       14 |       15 |                9.00 |      111,107,855.15 |    0.6% |      0.01 | `sixteen`
