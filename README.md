<h1>
  <a href="https://bencher.dev">
    <img
      src="https://bencher.dev/favicon/favicon.ico"
      alt="🐰 Bencher"
    />
  </a>
  Bencher
</h1>

[Bencher](https://bencher.dev) is a suite of [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking) tools designed to catch performance regressions in CI. That is, Bencher allows you to detect and prevent performance regressions _before_ they make it to production.

Bencher consists of:

- `bencher` CLI
- Bencher API Server
- Bencher Web UI

The best place to start is the [Bencher Quick Start](https://bencher.dev/docs/tutorial/quick-start) tutorial.

Though Bencher is open source, there is also a hosted version available [Bencher Cloud](https://bencher.dev).

<br />
<p align="center">
  <a href="https://discord.gg/yGEsdUh7R4">
    <img
      src="https://s3.amazonaws.com/public.bencher.dev/discord_invite.png"
      alt="Bencher Discord Server"
    />
  </a>
</p>


## Documentation

- Tutorial
  - [Quick Start](https://bencher.dev/docs/tutorial/quick-start)
- How To
  - [Install CLI](https://bencher.dev/docs/how-to/install-cli)
  - [Track Benchmarks](https://bencher.dev/docs/how-to/track-benchmarks)
  - [GitHub Actions](https://bencher.dev/docs/how-to/github-actions)
  - [GitLab CI/CD](https://bencher.dev/docs/how-to/gitlab-ci-cd)
- Explanation
  - [Benchmarking Overview](https://bencher.dev/docs/explanation/benchmarking)
  - [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters)
  - [Branch Selection](https://bencher.dev/docs/explanation/branch-selection)
  - [Thresholds & Alerts](https://bencher.dev/docs/explanation/thresholds)
  - [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking)
  - [Talks](https://bencher.dev/docs/explanation/talks)
- Reference
  - [REST API](https://bencher.dev/docs/reference/api)
  - [Server Config](https://bencher.dev/docs/reference/server-config)
  - [Prior Art](https://bencher.dev/docs/reference/prior-art)
  - [Roadmap](https://bencher.dev/docs/reference/roadmap)
  - [Changelog](https://bencher.dev/docs/reference/changelog)

## Supported Benchmark Harnesses

- {...} JSON
  - [Custom benchmark harness support](https://bencher.dev/docs/explanation/adapters)
- #️⃣ C#
  - [BenchmarkDotNet](https://github.com/dotnet/BenchmarkDotNet)
- ➕ C++
  - [Catch2](https://github.com/catchorg/Catch2)
  - [Google Benchmark](https://github.com/google/benchmark)
- 🕳 Go
  - [go test -bench](https://pkg.go.dev/testing#hdr-Benchmarks)
- ☕️ Java
  - [Java Microbenchmark Harness (JMH)](https://github.com/openjdk/jmh)
- 🕸 JavaScript
  - [Benchmark.js](https://github.com/bestiejs/benchmark.js)
  - [console.time](https://developer.mozilla.org/en-US/docs/Web/API/console/time)/[console.timeEnd](https://developer.mozilla.org/en-US/docs/Web/API/console/timeEnd)
- 🐍 Python
  - [airspeed velocity](https://github.com/airspeed-velocity/asv)
  - [pytest-benchmark](https://github.com/ionelmc/pytest-benchmark)
- ♦️ Ruby
  - [Benchmark](https://github.com/ruby/benchmark)
- 🦀 Rust
  - [libtest bench](https://doc.rust-lang.org/rustc/tests/index.html#benchmarks)
  - [Criterion](https://github.com/bheisler/criterion.rs)

For more details see the [explanation of benchmark harness adapters](https://bencher.dev/docs/explanation/adapters).

## Share Your Benchmarks

All public projects have their own [perf page](https://bencher.dev/perf). These results can easily be shared with an auto-updating perf image. Perfect for your README!

<p align="center">
<a href="https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674950400000"><img src="https://api.bencher.dev/v0/projects/bencher/perf/img?branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&metric_kind=latency&start_time=1674950400000&title=Benchmark+Adapter+Comparison" title="Benchmark Adapter Comparison" alt="Benchmark Adapter Comparison for Bencher - Bencher" /></a>
</p>

## Contributing

The easiest way to contribute is to open the repo in GitPod.
Everything you need will already be there!
It is best to connect to the GitPod instance with VS Code Desktop via SSH.
There is a hotkey set to tapping caps lock twice to prompt the web VS Code to create a VS Code Desktop SSH session.
For more details see the [GitPod docs here](https://www.gitpod.io/docs/references/ides-and-editors/vscode).
Once set up, both the UI and API should be built, running, and seeded at [localhost:3000](http://localhost:3000) and [localhost:61016](http://localhost:61016) respectively.

<br />
<p align="center">
  <a href="https://gitpod.io/#https://github.com/bencherdev/bencher">
    <img
      src="https://gitpod.io/button/open-in-gitpod.svg"
      alt="Bencher Gitpod"
    />
  </a>
</p>

## License
All content that resides under any directory or <a href="https://doc.rust-lang.org/cargo/reference/features.html">feature</a> named "plus" is licensed under the <a href="LICENSE-PLUS">Bencher Plus License</a>.

All other content is license under either of <a href="LICENSE-APACHE">Apache License, Version 2.0</a>
or <a href="LICENSE-MIT">MIT license</a> at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.