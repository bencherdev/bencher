<h1>
  <a href="https://bencher.dev">
    <img
      src="https://bencher.dev/favicon.svg"
      alt="🐰 Bencher"
      width=32
    />
  </a>
  Bencher
</h1>

[Bencher](https://bencher.dev) is a suite of [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/) tools.
Have you ever had a performance regression impact your users?
Bencher could have prevented that from happening.
Bencher allows you to detect and prevent performance regressions _before_ they hit production.

- **Run**: Run your benchmarks locally or in CI using your favorite benchmarking tools. The `bencher` CLI simply wraps your existing benchmark harness and stores its results.
- **Track**: Track the results of your benchmarks over time. Monitor, query, and graph the results using the Bencher web console based on the source branch, testbed, and measure.
- **Catch**: Catch performance regressions in CI. Bencher uses state of the art, customizable analytics to detect performance regressions before they make it to production.

For the same reasons that unit tests are run in CI to prevent feature regressions, benchmarks should be run in CI with Bencher to prevent performance regressions. Performance bugs are bugs!

<br />

Bencher consists of:

- `bencher` CLI
- Bencher API Server
- Bencher Web UI

<br />

The best place to start is the [Bencher Quick Start](https://bencher.dev/docs/tutorial/quick-start/) tutorial.

Though Bencher is open source, there is also a hosted version available [Bencher Cloud](https://bencher.dev/).

<br />
<p align="center">
  <a href="https://discord.gg/yGEsdUh7R4">
    <img
      src="https://s3.amazonaws.com/public.bencher.dev/chat/discord_invite.png"
      alt="Bencher Discord Server"
    />
  </a>
</p>

> 🐰 [Use the GitHub Action with your project](#github-actions)

## Documentation

- Tutorial
  - [Quick Start](https://bencher.dev/docs/tutorial/quick-start/)
  - [Docker](https://bencher.dev/docs/tutorial/docker/)
- How To
  - [Install CLI](https://bencher.dev/docs/how-to/install-cli/)
  - [Track Benchmarks](https://bencher.dev/docs/how-to/track-benchmarks/)
  - [GitHub Actions](https://bencher.dev/docs/how-to/github-actions/)
  - [GitLab CI/CD](https://bencher.dev/docs/how-to/gitlab-ci-cd/)
- Explanation
  - [Benchmarking Overview](https://bencher.dev/docs/explanation/benchmarking/)
  - [`bencher run`](https://bencher.dev/docs/explanation/bencher-run/)
  - [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters/)
  - [Branch Selection](https://bencher.dev/docs/explanation/branch-selection/)
  - [Thresholds & Alerts](https://bencher.dev/docs/explanation/thresholds/)
  - [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/)
  - [Talks](https://bencher.dev/docs/explanation/talks/)
- Reference
  - [REST API](https://bencher.dev/docs/reference/api/)
  - [Architecture](https://bencher.dev/docs/reference/architecture/)
  - [Server Config](https://bencher.dev/docs/reference/server-config/)
  - [Prior Art](https://bencher.dev/docs/reference/prior-art/)
  - [Roadmap](https://bencher.dev/docs/reference/roadmap/)
  - [Changelog](https://bencher.dev/docs/reference/changelog/)

🌐 Also available in:

- [简体中文](https://bencher.dev/zh/docs/)
- [Español](https://bencher.dev/es/docs/)
- [Português do Brasil](https://bencher.dev/pt/docs/)
- [Русский](https://bencher.dev/ru/docs/)
- [日本語](https://bencher.dev/ja/docs/)
- [Français](https://bencher.dev/fr/docs/)
- [Deutsch](https://bencher.dev/de/docs/)
- [한국어](https://bencher.dev/ko/docs/)

## Supported Benchmark Harnesses

- {...} JSON
  - [Custom benchmark harness support](https://bencher.dev/docs/explanation/adapters/#-json)
- #️⃣ C#
  - [BenchmarkDotNet](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F%E2%83%A3-c-dotnet)
- ➕ C++
  - [Catch2](https://bencher.dev/docs/explanation/adapters/#-c-catch2)
  - [Google Benchmark](https://bencher.dev/docs/explanation/adapters/#-c-google)
- 🕳 Go
  - [go test -bench](https://bencher.dev/docs/explanation/adapters/#-go-bench)
- ☕️ Java
  - [Java Microbenchmark Harness (JMH)](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F-java-jmh)
- 🕸 JavaScript
  - [Benchmark.js](https://bencher.dev/docs/explanation/adapters/#-javascript-benchmark)
  - [console.time/console.timeEnd](https://bencher.dev/docs/explanation/adapters/#-javascript-time)
- 🐍 Python
  - [airspeed velocity](https://bencher.dev/docs/explanation/adapters/#-python-asv)
  - [pytest-benchmark](https://bencher.dev/docs/explanation/adapters/#-python-pytest)
- ♦️ Ruby
  - [Benchmark](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F-ruby-benchmark)
- 🦀 Rust
  - [libtest bench](https://bencher.dev/docs/explanation/adapters/#-rust-bench)
  - [Criterion](https://bencher.dev/docs/explanation/adapters/#-rust-criterion)
  - [Iai](https://bencher.dev/docs/explanation/adapters/#-rust-iai)
- ❯_ Shell
  - [Hyperfine](https://bencher.dev/docs/explanation/adapters/#_%EF%B8%8F-shell-hyperfine)

👉 For more details see the [explanation of benchmark harness adapters](https://bencher.dev/docs/explanation/adapters/).

## GitHub Actions

Install the Bencher CLI using the [GitHub Action](https://github.com/marketplace/actions/bencher-cli),
and use it for [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/) in your project.

```yaml
name: Continuous Benchmarking with Bencher
on:
  push:
    branches: main
jobs:
  benchmark_with_bencher:
    name: Benchmark with Bencher
    runs-on: ubuntu-latest
    env:
      - BENCHER_PROJECT: my-project-slug
      - BENCHER_API_TOKEN: ${{ secrets.BENCHER_API_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - uses: bencherdev/bencher@main
      - run: bencher run "bencher mock"
```

👉 For more details see the [explanation of how to use GitHub Actions](https://bencher.dev/docs/how-to/github-actions/).

### Repository Secrets

Add `BENCHER_API_TOKEN` to you **Repository** secrets (ex: `Repo -> Settings -> Secrets and variables -> Actions -> New repository secret`). You can find your API tokens by running `bencher token ls --user my-user-slug` or by going to the Bencher Console (ex: `https://bencher.dev/console/users/my-user-slug/tokens`).

### Error on Alert

You can set the `bencher run` CLI subcommand to error
if [an Alert is generated](https://bencher.dev/docs/explanation/thresholds/) with the `--err` flag.

```bash
bencher run --err "bencher mock"
```

👉 For more details see the [explanation of `bencher run`](https://bencher.dev/docs/explanation/bencher-run/#--err).

### Comment on PRs

You can set the `bencher run` CLI subcommand to comment on a PR with the `--github-actions` argument.

```bash
bencher run --github-actions "${{ secrets.GITHUB_TOKEN }}" "bencher mock"
```

👉 For more details see the [explanation of `bencher run`](https://bencher.dev/docs/explanation/bencher-run/#--github-actions/).

### Example PR Comment

<br />

<h1><a href="https://bencher.dev"><img src="https://bencher.dev/favicon.svg" width="32" height="32" alt="🐰" /></a>Bencher</h1><table><tr><td>Report</td><td>Tue, December  5, 2023 at 00:16:53 UTC</td></tr><tr><td>Project</td><td><a href="https://bencher.dev/perf/bencher">Bencher</a></td></tr><tr><td>Branch</td><td>254/merge</td></tr><tr><td>Testbed</td><td>ubuntu-latest</td></tr></table><table><tr><th>Benchmark</th><th>Latency</th><th>Latency Results<br/>nanoseconds (ns) | (Δ%)</th><th>Latency Upper Boundary<br/>nanoseconds (ns) | (%)</th></tr><tr><td>Adapter::Json</td><td>🚨 (<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=e93b3d71-8499-4fae-bb7c-4e540b775714&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">view plot</a> | <a href="https://bencher.dev/perf/bencher/alerts/91ee27a7-2aee-41fe-b037-80b786f26cd5">view alert</a>)</td><td>3445.600 (+1.52%)</td><td>3362.079 (102.48%)</td></tr><tr><td>Adapter::Magic (JSON)</td><td>✅ (<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3bfd5887-83ec-4e62-8690-02855a38fbc9&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">view plot</a>)</td><td>3431.400 (+0.69%)</td><td>3596.950 (95.40%)</td></tr><tr><td>Adapter::Magic (Rust)</td><td>✅ (<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">view plot</a>)</td><td>22095.000 (-0.83%)</td><td>24732.801 (89.33%)</td></tr><tr><td>Adapter::Rust</td><td>✅ (<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=5655ed2a-3e45-4622-bdbd-39cdd9837af8&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">view plot</a>)</td><td>2305.700 (-2.76%)</td><td>2500.499 (92.21%)</td></tr><tr><td>Adapter::RustBench</td><td>✅ (<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=1db23e93-f909-40aa-bf42-838cc7ae05f5&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">view plot</a>)</td><td>2299.900 (-3.11%)</td><td>2503.419 (91.87%)</td></tr></table><br/><small><a href="https://bencher.dev">Bencher - Continuous Benchmarking</a></small><br/><small><a href="https://bencher.dev/perf/bencher">View Public Perf Page</a></small><br/><small><a href="https://bencher.dev/docs">Docs</a> | <a href="https://bencher.dev/repo">Repo</a> | <a href="https://bencher.dev/chat">Chat</a> | <a href="https://bencher.dev/help">Help</a></small>

### Specify CLI Version

There is also an optional `version` argument to specify an exact version of the Bencher CLI to use.
Otherwise, it will default to using the latest CLI version.

```yaml
- uses: bencherdev/bencher@main
  with:
    version: 0.3.0
```

Specify an exact version if using Bencher _Self-Hosted_.
Do **not** specify an exact version if using Bencher _Cloud_ as there are still occasional breaking changes.

## Share Your Benchmarks

All public projects have their own [perf page](https://bencher.dev/perf). These results can easily be shared with an auto-updating perf image. Perfect for your README!

<p align="center">
  <a href="https://bencher.dev/perf/bencher?key=true&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674950400000">
    <img
      src="https://api.bencher.dev/v0/projects/bencher/perf/img?branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1674950400000&title=Benchmark+Adapter+Comparison"
      title="Benchmark Adapter Comparison"
      alt="Benchmark Adapter Comparison for Bencher - Bencher"
    />
  </a>
</p>

## Contributing

The easiest way to contribute is to open this repo as a [Dev Container](https://containers.dev) in [VSCode](https://code.visualstudio.com/download) by simply clicking one of the buttons below.
Everything you need will already be there!
Once set up, both the UI and API should be built, running, and seeded at [localhost:3000](http://localhost:3000) and [localhost:61016](http://localhost:61016) respectively.
To make any changes to the UI or API though, you will have to exit the startup process and restart the UI and API yourself.


<br />
<p align="center">
  <a href="https://vscode.dev/redirect?url=vscode://ms-vscode-remote.remote-containers/cloneInVolume?url=https://github.com/bencherdev/bencher">
    <img
      src="https://img.shields.io/static/v1?label=Local%20Dev%20Container&message=Open&color=orange&logo=visualstudiocode&style=for-the-badge"
      alt="Bencher VSCode Dev Container"
    />
  </a>
</p>

<p align="center">
  <a href="https://github.dev/bencherdev/bencher">
    <img
      src="https://img.shields.io/static/v1?label=GitHub%20Codespaces&message=Open&color=orange&logo=github&style=for-the-badge"
      alt="Bencher GitHub Codespaces"
    />
  </a>
</p>

There is also a [pre-built image from CI](https://github.com/orgs/bencherdev/packages/container/package/bencher-dev-container) available for each branch: `ghcr.io/bencherdev/bencher-dev-container`

## License
All content that resides under any directory or [feature](https://doc.rust-lang.org/cargo/reference/features.html) named "plus" is licensed under the [Bencher Plus License](license/LICENSE-PLUS).

All other content is licensed the [Apache License, Version 2.0](license/LICENSE-APACHE) or [MIT License](license/LICENSE-MIT) at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
