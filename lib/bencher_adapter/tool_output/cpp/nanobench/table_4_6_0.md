Note: perf_event_open failed, so the ins/op, cyc/op, IPC, bra/op and miss% columns are missing.
This is usually a container or VM without virtualized performance counters, or
/proc/sys/kernel/perf_event_paranoid being too restrictive.


| relative |           ns/insert |            insert/s |    err% |     total | Containers
|---------:|--------------------:|--------------------:|--------:|----------:|:-----------
|   100.0% |               73.21 |       13,659,133.57 |   12.8% |      0.01 | :wavy_dash: `std::set insert` (Unstable with ~16,839.5 iters. Increase `minEpochIterations` to e.g. 168395)
|18,316.4% |                0.40 |    2,501,868,075.37 |    6.5% |      0.01 | :wavy_dash: `std::vector push_back x1000` (Unstable with ~2,711.9 iters. Increase `minEpochIterations` to e.g. 27119)
|        - |                   - |                   - |       - |         - | :boom: `empty body` (iterations overflow. Maybe your code got optimized away?)

|               ns/op |                op/s |    err% |     total | Context
|--------------------:|--------------------:|--------:|----------:|:--------
|                6.39 |      156,524,672.30 |    2.5% |      0.01 | `vector fill`
|               13.89 |       71,996,642.94 |    1.5% |      0.01 | `vector fill`

| relative |              95% CI |               ns/op |                op/s |    err% |     total | Compare
|---------:|--------------------:|--------------------:|--------------------:|--------:|----------:|:--------
|   100.0% |                     |                1.13 |      882,084,466.44 |    1.3% |      0.01 | `mt19937`
|    43.5% |      42.9% .. 43.9% |                2.60 |      384,823,923.70 |    0.2% |      0.03 | `minstd`

  Summary
    `minstd` ran 2.30x slower than `mt19937`
    95% CI [2.28 .. 2.33], 22 paired rounds, interleaved
