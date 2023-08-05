<h1>
  <a href="https://bencher.dev">
    <img
      src="https://bencher.dev/favicon/favicon.ico"
      alt="🐰 Bencher"
    />
  </a>
  Bencher
</h1>

[Bencher](https://bencher.dev) is a suite of [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking) tools.
Have you ever had a performance regression impact your users?
Bencher could have prevented that from happening.
Bencher allows you to detect and prevent performance regressions _before_ they make it to production.

- **Run**: Run your benchmarks locally or in CI using your favorite benchmarking tools. The `bencher` CLI simply wraps your existing benchmark harness and stores its results.
- **Track**: Track the results of your benchmarks over time. Monitor, query, and graph the results using the Bencher web console based on the source branch, testbed, and metric kind.
- **Catch**: Catch performance regressions in CI. Bencher uses state of the art, customizable analytics to detect performance regressions before they make it to production.

For the same reasons that unit tests are run in CI to prevent feature regressions, benchmarks should also be run in CI with Bencher to prevent performance regressions. Performance bugs are bugs!

<br />

Bencher consists of:

- `bencher` CLI
- Bencher API Server
- Bencher Web UI

<br />

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

> 🐰 [Use the GitHub Action with your project](#github-actions)

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
  - [`bencher run`](https://bencher.dev/docs/explanation/bencher-run)
  - [Branch Selection](https://bencher.dev/docs/explanation/branch-selection)
  - [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters)
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
  - [Iai](https://github.com/bheisler/iai)

For more details see the [explanation of benchmark harness adapters](https://bencher.dev/docs/explanation/adapters).

## Share Your Benchmarks

All public projects have their own [perf page](https://bencher.dev/perf). These results can easily be shared with an auto-updating perf image. Perfect for your README!

<p align="center">
  <a href="https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674950400000">
    <img
      src="https://api.bencher.dev/v0/projects/bencher/perf/img?branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&metric_kind=latency&start_time=1674950400000&title=Benchmark+Adapter+Comparison"
      title="Benchmark Adapter Comparison"
      alt="Benchmark Adapter Comparison for Bencher - Bencher"
    />
  </a>
</p>

## GitHub Actions

Install the Bencher CLI using the [GitHub Action](https://github.com/marketplace/actions/bencher-cli),
and use it for [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking) in your project.
See [how to use GitHub Actions](https://bencher.dev/docs/how-to/github-actions) for more details.

```yaml
name: Continuous Benchmarking with Bencher
on: [push]
jobs:
  benchmark_with_bencher:
    name: Benchmark with Bencher
    runs-on: ubuntu-latest
    env:
      - BENCHER_API_TOKEN: ${{ secrets.BENCHER_API_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - uses: bencherdev/bencher@main
      - run: bencher run --project my-project-slug "bencher mock"
```

### Repository Secrets

Add `BENCHER_API_TOKEN` to you **Repository** secrets (ex: `https://github.com/my-user-slug/my-repo/settings/secrets/actions`). You can find your API tokens by running `bencher token ls --user my-user-slug` or by going to the Bencher Console (ex: `https://bencher.dev/console/users/my-user-slug/tokens`).

### Error on Alert

You can set the [`bencher run` CLI subcommand](https://bencher.dev/docs/explanation/bencher-run) to error
if [an Alert is generated](https://bencher.dev/docs/explanation/thresholds) with the `--err` flag.

```yaml
bencher run --project my-project-slug --err "bencher mock"
```

### Comment on PRs

You can set the [`bencher run` CLI subcommand](https://bencher.dev/docs/explanation/bencher-run) to comment on a PR with the `--github-actions` argument.

```yaml
bencher run --project my-project-slug --github-actions ${{ secrets.GITHUB_TOKEN }} "bencher mock"
```

If you want to only show results when [a Threshold is set](https://bencher.dev/docs/explanation/thresholds) use the `--ci-only-thresholds` flag.

```yaml
bencher run --project my-project-slug --github-actions ${{ secrets.GITHUB_TOKEN }} --ci-only-thresholds "bencher mock"
```

Or if you only want it to start commenting once there is [an Alert is generated](https://bencher.dev/docs/explanation/thresholds) use the `--ci-only-on-alert` flag.

```yaml
bencher run --project my-project-slug --github-actions ${{ secrets.GITHUB_TOKEN }} --ci-only-on-alert "bencher mock"
```

You can also use all three together!

```yaml
bencher run --project my-project-slug --github-actions ${{ secrets.GITHUB_TOKEN }} --ci-only-thresholds --ci-only-on-alert "bencher mock"
```

### Example PR Comment

<br />
<h1><a href="https://bencher.dev/"><img src="https://s3.amazonaws.com/public.bencher.dev/bencher_rabbit.svg" width="32" height="32" alt="🐰" /></a>Bencher</h1>
<table>
    <tr>
        <td>Report</td>
        <td><a href="https://bencher.dev/console/projects/bencher/reports/738fbe06-8b2c-4c94-b8f3-1f0b88472fca">Wed, August 2, 2023 at 16:07:31 UTC</a></td>
    </tr>
    <tr>
        <td>Project</td>
        <td><a href="https://bencher.dev/console/projects/bencher">Bencher</a></td>
    </tr>
    <tr>
        <td>Branch</td>
        <td><a href="https://bencher.dev/console/projects/bencher/branches/162-merge">162/merge</a></td>
    </tr>
    <tr>
        <td>Testbed</td>
        <td><a href="https://bencher.dev/console/projects/bencher/testbeds/ubuntu-latest">ubuntu-latest</a></td>
    </tr>
</table>
<table>
    <tr>
        <th>Benchmark</th>
        <th><a href="https://bencher.dev/console/projects/bencher/metric-kinds/latency">Latency</a></th>
    </tr>
    <tr>
        <td><a href="https://bencher.dev/console/projects/bencher/benchmarks/e93b3d71-8499-4fae-bb7c-4e540b775714">JsonAdapter::Json</a></td>
        <td>🚨 (<a href="https://bencher.dev/console/projects/bencher/perf?metric_kind=latency&branches=a90d5b4f-047e-4dbe-8bce-1516a30df049&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=e93b3d71-8499-4fae-bb7c-4e540b775714&upper_boundary=true">view plot</a> | <a href="https://bencher.dev/console/projects/bencher/alerts/90f565dd-202c-4a82-b852-aaa8f3e98da4">view alert</a>)</td>
    </tr>
    <tr>
        <td><a href="https://bencher.dev/console/projects/bencher/benchmarks/3bfd5887-83ec-4e62-8690-02855a38fbc9">JsonAdapter::Magic (JSON)</a></td>
        <td>✅ (<a href="https://bencher.dev/console/projects/bencher/perf?metric_kind=latency&branches=a90d5b4f-047e-4dbe-8bce-1516a30df049&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3bfd5887-83ec-4e62-8690-02855a38fbc9&upper_boundary=true">view plot</a>)</td>
    </tr>
    <tr>
        <td><a href="https://bencher.dev/console/projects/bencher/benchmarks/3525f177-fc8f-4a92-bd2f-dda7c4e15699">JsonAdapter::Magic (Rust)</a></td>
        <td>✅ (<a href="https://bencher.dev/console/projects/bencher/perf?metric_kind=latency&branches=a90d5b4f-047e-4dbe-8bce-1516a30df049&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699&upper_boundary=true">view plot</a>)</td>
    </tr>
    <tr>
        <td><a href="https://bencher.dev/console/projects/bencher/benchmarks/5655ed2a-3e45-4622-bdbd-39cdd9837af8">JsonAdapter::Rust</a></td>
        <td>✅ (<a href="https://bencher.dev/console/projects/bencher/perf?metric_kind=latency&branches=a90d5b4f-047e-4dbe-8bce-1516a30df049&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=5655ed2a-3e45-4622-bdbd-39cdd9837af8&upper_boundary=true">view plot</a>)</td>
    </tr>
    <tr>
        <td><a href="https://bencher.dev/console/projects/bencher/benchmarks/1db23e93-f909-40aa-bf42-838cc7ae05f5">JsonAdapter::RustBench</a></td>
        <td>🚨 (<a href="https://bencher.dev/console/projects/bencher/perf?metric_kind=latency&branches=a90d5b4f-047e-4dbe-8bce-1516a30df049&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=1db23e93-f909-40aa-bf42-838cc7ae05f5&upper_boundary=true">view plot</a> | <a href="https://bencher.dev/console/projects/bencher/alerts/f84550f3-972d-4f84-b204-3a9c292d46a2">view alert</a>)</td>
    </tr>
</table>
<br />
<small>
  <a href="https://bencher.dev">Bencher - Continuous Benchmarking</a>
</small>
<br />
<small>
  <a href="https://bencher.dev/perf/bencher">View Public Perf Page</a>
</small>
<br />
<small>
  <a href="https://bencher.dev/docs">Docs</a> | <a href="https://bencher.dev/repo">Repo</a> | <a href="https://bencher.dev/chat">Chat</a> | <a href="https://bencher.dev/help">Help</a>
</small>
<div id="bencher.dev/projects/6bd4b7f2-d850-4028-8d1b-edd18a9cae1d/testbeds/0d991aac-b241-493a-8b0f-8d41419455d2"></div>
<br />

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

## License
All content that resides under any directory or [feature](https://doc.rust-lang.org/cargo/reference/features.html) named "plus" is licensed under the [Bencher Plus License](license/LICENSE-PLUS).

All other content is license under either of [Apache License, Version 2.0](license/LICENSE-APACHE) or [MIT license](license/LICENSE-MIT) at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
