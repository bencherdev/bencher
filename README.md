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
- Bencher Console Web UI

<br />

Though Bencher is open source, there is also a hosted version available [Bencher Cloud](https://bencher.dev/).

The best place to start is the [Bencher Quick Start](https://bencher.dev/docs/tutorial/quick-start/) tutorial.

<br />
<p align="center">
  <a href="https://bencher.dev/docs/tutorial/quick-start/">
    <img
      src="https://s3.amazonaws.com/public.bencher.dev/github/continuous-benchmarking.png"
      alt="Start Continuous Benchmarking"
    />
  </a>
</p>

> 🐰 [Use the GitHub Action with your project](#github-actions)

## Documentation

- Tutorial
  - [Quick Start](https://bencher.dev/docs/tutorial/quick-start/)
  - [Docker Self-Hosted](https://bencher.dev/docs/tutorial/docker/)
- How To
  - [Install CLI](https://bencher.dev/docs/how-to/install-cli/)
  - [Track Benchmarks in CI](https://bencher.dev/docs/how-to/track-benchmarks/)
  - [GitHub Actions](https://bencher.dev/docs/how-to/github-actions/)
  - [GitLab CI/CD](https://bencher.dev/docs/how-to/gitlab-ci-cd/)
  - [Track Custom Benchmarks](https://bencher.dev/docs/how-to/track-custom-benchmarks/)
  - [Track File Size](https://bencher.dev/docs/how-to/track-file-size/)
  - [Self-Hosted GitHub App](https://bencher.dev/docs/how-to/github-app/)
- Explanation
  - [Benchmarking Overview](https://bencher.dev/docs/explanation/benchmarking/)
  - [`bencher run`](https://bencher.dev/docs/explanation/bencher-run/)
  - [Branch Selection](https://bencher.dev/docs/explanation/branch-selection/)
  - [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters/)
  - [Thresholds & Alerts](https://bencher.dev/docs/explanation/thresholds/)
  - [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/)
  - [Talks](https://bencher.dev/docs/explanation/talks/)
- Reference
  - [REST API](https://bencher.dev/docs/api/)
  - [Architecture](https://bencher.dev/docs/reference/architecture/)
  - [Server Config](https://bencher.dev/docs/reference/server-config/)
  - [Bencher Metric Format](https://bencher.dev/docs/reference/bencher-metric-format/)
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
  - [File Size (Binary Size)](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F-file-size)
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
  - [Iai-Callgrind](https://bencher.dev/docs/explanation/adapters/#-rust-iai-callgrind)
- ❯_ Shell
  - [Hyperfine](https://bencher.dev/docs/explanation/adapters/#_%EF%B8%8F-shell-hyperfine)

👉 For more details see the [explanation of benchmark harness adapters](https://bencher.dev/docs/explanation/adapters/).

## Showcase

<table>
  <tr>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/ccf?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=d5004f0a-5dbe-42bb-a821-1f55704d6ec2&testbeds=1e6f6a27-eb58-4f16-8d01-0148fbaed70e&benchmarks=3bae8305-29e0-4e5f-8157-01f8f471b408&measures=bc9fb376-9a85-478a-97fd-ebd7703c9663&start_time=1715185355000&end_time=1717777355000&clear=true&tab=benchmarks">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/microsoft.png"
            alt="Microsoft"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Microsoft CCF</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/rustls-821705769?key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=branches&measures=013468de-9c37-4605-b363-aebbbf63268d&branches=28fae530-2b53-4482-acd4-47e16030d54f&testbeds=62ed31c3-8a58-479c-b828-52521ed67bee&benchmarks=bd25f73c-b2b9-4188-91b4-f632287c0a1b%2C8d443816-7a23-40a1-a54c-59de911eb517%2C42edb37f-ca91-4984-8835-445514575c85&start_time=1704067200000&notify_kind=alert&notify_text=Learn%20more%20about%20continuous%20benchmarking%20for%20the%20Rustls%20project.&notify_timeout=2147483647&notify_link_url=https%3A%2F%2Fbencher.dev%2Flearn%2Fcase-study%2Frustls%2F&notify_link_text=Read%20the%20case%20study">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/rustls-rust-tls.png"
            alt="Rustls TLS Library"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Rustls</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/diesel?key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=benchmarks&measures=2d3bd4cd-c4d4-4aa1-9e60-47e51e2b9dde&branches=bf9a5209-6524-45e3-af26-b8f98eee3bad&testbeds=4e5c3c90-920c-4741-8cf7-aaed4e16e9a5&benchmarks=5dfa78a5-7785-4d33-a336-aab5fff43372%2Cf65ec533-abf5-443e-a0d8-e4a583c5779e%2C0c1bcad9-2100-4170-9bc7-96a3b89071b9%2Ccee41d01-30db-4acc-8727-0d0b4ccbe216%2C6d23685f-e082-4913-8c22-14311030d130&notify_kind=alert&notify_text=Learn%20more%20about%20continuous%20benchmarking%20for%20the%20Diesel%20project.&notify_timeout=2147483647&notify_link_url=https%3A%2F%2Fbencher.dev%2Flearn%2Fcase-study%2Fdiesel%2F&notify_link_text=Read%20the%20case%20study">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/diesel.svg"
            alt="Diesel"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Diesel</p>
    </td>
  </tr>
  <tr>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/hydra-postgres?key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=6&clear=true&tab=branches&measures=c20a9c30-e20a-45b7-bba5-4a6e940f951f&branches=e6bcbe0c-210d-4ab1-8fe4-5d9498800980&testbeds=1d3283b3-3e52-4dd0-a018-fb90c9361a2e&benchmarks=b31c3185-9701-4576-9fd7-288aea5cc7e4%2Cc4efd5bb-f4c4-4b75-9137-f2a841c04cfe%2C6e050650-ad8a-4043-b62c-a39e0e202bfe%2Cec575db9-3c10-4122-af8f-a062be36a198">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/hydra-db.svg"
            alt="Hydra Database"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Hydra Database</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/greptimedb?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=8dcbda4a-c239-4a9e-8399-4fc699f775b4&branches=3b46708f-b87f-4f52-b1bb-1d9cc7bfee2d&testbeds=6d3be02f-9efe-4e47-8a5d-e389c228172d&benchmarks=da5c8cbe-9aef-431e-9168-11ef0821c8db%2Cbb7ce469-5c34-4a69-ab2f-d9769ca5be2a&measures=a2f1689d-44d5-4d5e-863f-47d285cedf97&start_time=1707524593000&end_time=1710116593000&clear=true">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/greptimedb.svg"
            alt="GreptimeDB"
            width="300px"
          />
        </a>
      </p>
      <p align="center">GreptimeDB</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/tailcall?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&branches=3646cfed-fd77-417e-b8d5-90eab450e855&testbeds=5823e8f8-162f-4a86-862d-3ed9b3415a75&benchmarks=5022fcf2-e392-4dc6-8b62-cb2da9a6e36a%2Cd1499469-f2dc-4b38-91ba-83ecf11ce678%2C851fc472-d9d7-42b8-ba91-b0f90e3c9909%2Cdbea7f22-5076-4a91-a83e-bb2cadddb069&measures=d6846b7a-7a7a-4e2e-91a1-131232a131e3&start_time=1710981217000&end_time=1713573818000&clear=true&upper_boundary=false&range=version&tab=branches">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/tailcall.svg"
            alt="Tailcall"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Tailcall</p>
    </td>
  </tr>
  <tr>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/ratatui-org?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=95ce51f3-9a78-41e8-8700-562f11680798&testbeds=0615b230-cbf8-4ea6-8e2e-616c282b102a&measures=b917dd68-60ef-41c6-8ce9-2164eba4f46b&start_time=1720841447000&end_time=1723434422000&clear=true&tab=benchmarks&branches_search=main&benchmarks_search=barchart%2F&benchmarks=5695514c-6501-44a4-9a43-9de69078be9c%2C7bada371-e16a-475b-9424-af842fd2dd70%2Cadb521a6-df19-4ee9-af93-e783b69a4dc0&upper_boundary=false&lower_boundary=false">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/ratatui.png"
            alt="Ratatui"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Ratatui</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/poolifier?key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=branches&branches=977f91aa-2157-4e5b-a4dc-e1d8c3ece8af&testbeds=12203dc4-c6e4-439b-bb2b-a5d4e227e4f5&measures=73517df3-f327-4853-9546-a8b61381b5e2&benchmarks=2515bbd1-81c8-4ab2-8746-135c6fa638b6%2Cf96b89da-378e-42a4-bc16-2034c1e16b3a%2Cdc1c353d-1da9-4940-af1f-d0cbdef98b03%2Cbe79f393-70f3-4a94-b377-f7b80e345461&start_time=1704067200000&benchmarks_search=FixedClusterPool+with+FAIR_SHARE">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/poolifier.png"
            alt="Poolifier"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Poolifier</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/core-crypto-mmbtki3h?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=cd6b82fc-bbfb-4680-afa6-ab88ca62a1ef&testbeds=7f837718-cf29-423f-bd13-2b516ec88cda&measures=c1f87d1c-d949-4bf4-8b76-eb782e882d0e&start_time=1719668529000&end_time=1722261285000&clear=true&tab=benchmarks&benchmarks_search=6010&benchmarks=a4cefec8-6548-4e20-a7c1-75456b7ea925%2C0c73af64-460b-4082-a73b-77e3a980606d">
          <img
            src="https://s3.amazonaws.com/public.bencher.dev/case-study/wire.svg"
            alt="Wire"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Wire</p>
    </td>
  </tr>
</table>

👉 Checkout [all public projects](https://bencher.dev/perf).

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
      BENCHER_PROJECT: my-project-slug
      BENCHER_API_TOKEN: ${{ secrets.BENCHER_API_TOKEN }}
    steps:
      - uses: actions/checkout@v4
      - uses: bencherdev/bencher@main
      - run: bencher run "bencher mock"
```

Supported Operating Systems:
- Linux (x86_64 & ARM64)
- MacOS (x86_64 & ARM64)
- Windows (x86_64 & ARM64)

<br />

👉 For more details see the [explanation of how to use GitHub Actions](https://bencher.dev/docs/how-to/github-actions/).

### Repository Secrets

Add `BENCHER_API_TOKEN` to you **Repository** secrets (ex: `Repo -> Settings -> Secrets and variables -> Actions -> New repository secret`). You can find your API tokens by running `bencher token list my-user-slug` or [view them in the Bencher Console](https://bencher.dev/console/users/tokens).

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
    version: 0.4.5
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

#### 🐰 All pull requests should target the `devel` branch

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

All other content is licensed under the [Apache License, Version 2.0](license/LICENSE-APACHE) or [MIT License](license/LICENSE-MIT) at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
