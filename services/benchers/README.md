# Bencher for Rust (`benchers`)

[Bencher](https://bencher.dev) is a suite of [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/) tools.
Have you ever had a performance regression impact your users?
Bencher could have prevented that from happening.
Bencher allows you to detect and prevent performance regressions _before_ they merge.

- **Run**: Run your benchmarks locally or in CI using the _exact same_ bare metal runners and your favorite benchmarking tools. The `bencher` CLI orchestrates running your benchmarks on bare metal and stores the results.
- **Track**: Track the results of your benchmarks over time. Monitor, query, and graph the results using the Bencher web console based on the source branch, testbed, benchmark, and measure.
- **Catch**: Catch performance regressions locally or in CI using the _exact same_ bare metal hardware. Bencher uses state of the art, customizable analytics to detect performance regressions before they merge.

For the same reasons that unit tests are run to prevent feature regressions, benchmarks should be run with Bencher to prevent performance regressions. Performance bugs are bugs!

## Supported Benchmark Harnesses

- [libtest bench](https://bencher.dev/docs/explanation/adapters/#-rust-bench)
- [Criterion](https://bencher.dev/docs/explanation/adapters/#-rust-criterion)
- [Iai](https://bencher.dev/docs/explanation/adapters/#-rust-iai)
- [Iai-Callgrind](https://bencher.dev/docs/explanation/adapters/#-rust-iai-callgrind)
