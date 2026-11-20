
| relative |           ns/insert |            insert/s |    err% |     total | Containers
|---------:|--------------------:|--------------------:|--------:|----------:|:-----------
|   100.0% |               83.98 |       11,907,488.65 |   14.7% |      0.01 | :wavy_dash: `std::set insert` (Unstable with ~15,289.9 iters. Increase `minEpochIterations` to e.g. 152899)
|    19.9% |                0.42 |    2,370,922,161.12 |    1.5% |      0.01 | `std::vector push_back x1000`
|     7.5% |         100 |            1,124.62 |          889,190.16 |    0.1% |      0.01 | `noisy | name with spaces`

|               ns/op |                op/s |    err% |     total | Strings
|--------------------:|--------------------:|--------:|----------:|:--------
|               11.25 |       88,891,440.21 |    1.0% |      0.01 | `std::string append`
