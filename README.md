<h1>
  <a href="https://bencher.dev">
    <img
      src="https://bencher.dev/favicon.svg"
      aria-label="🐰 Bencher"
      width=32
    />
  </a>
  Bencher
</h1>

**Continuous benchmarking on bare metal**

[Bencher](https://bencher.dev) catches performance regressions before they merge.

<p align="center">
  <a href="https://bencher.dev">
    <img
      src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/github/regression-chart.svg"
      alt="Animated chart showing benchmark results with a performance regression spike that triggers an alert, followed by a fix back to baseline"
      width="800"
    />
  </a>
</p>

Typical CI runners have **>30% variance**. Bencher Bare Metal runners have **<2%**. When a number moves, it means something.

<p>
  <a href="https://bencher.dev/auth/signup"><b>Benchmark for free →</b></a>
  <a href="https://bencher.dev/docs/tutorial/bare-metal/">Bare Metal Quickstart</a>
</p>

<p>
  Trusted by engineers at
  <a href="#showcase">Google</a>,
  <a href="#showcase">Microsoft</a>,
  <a href="#showcase">GitLab</a>,
  <a href="#showcase">Mozilla</a>,
  and <a href="#showcase">The Linux Foundation</a>.
</p>

## The Problem

Local benchmarks aren't reproducible. Every check means stopping work to pull the baseline branch and wait on a comparison. Most engineers skip it.

CI runners are shared and noisy. Noisy benchmarks train engineers to ignore alerts. Performance regressions silently ship.

If you can't tell a real regression from noise, the results are worthless.
So teams stop looking.

## How Bencher Works

1. **Run**: Run your benchmarks locally or in CI using the _exact same_ bare metal runners and your favorite benchmarking tools. The [`bencher` CLI](https://bencher.dev/docs/how-to/install-cli/) orchestrates running your benchmarks on bare metal and stores the results.
2. **Track**: Track the results of your benchmarks over time. Monitor, query, and graph the results using the Bencher web console based on the source branch, testbed, benchmark, and measure.
3. **Catch**: Catch performance regressions locally or in CI using the _exact same_ bare metal hardware. Bencher uses state of the art, customizable analytics to detect performance regressions before they merge.

For the same reasons that unit tests are run to prevent feature regressions, benchmarks should be run with Bencher to prevent performance regressions.
Performance bugs are bugs!

## Every benchmark run lands as a PR comment

No dashboards to remember. No manual runs. Regressions fail the build.

<h2><a href="https://bencher.dev/perf/bencher/reports/36a1eeff-57f5-4b99-b058-8c9c240a9f2c?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher"><img src="https://bencher.dev/favicon.svg" width="24" height="24" alt="🐰" /> Bencher Report</a></h2><table><tr><td>Branch</td><td><a href="https://bencher.dev/perf/bencher/branches/254-merge?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">254/merge</a></td></tr><tr><td>Testbed</td><td><a href="https://bencher.dev/perf/bencher/testbeds/ubuntu-latest?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">ubuntu-latest</a></td></tr></table><blockquote><b>🚨 1 ALERT:</b> Threshold Boundary Limit exceeded!</blockquote><table><thead><tr><th>Benchmark</th><th>Measure<br/>Units</th><th>View</th><th>Benchmark Result<br/>(Result Δ%)</th><th>Upper Boundary<br/>(Limit %)</th></tr></thead><tbody><tr><td><a href="https://bencher.dev/perf/bencher/benchmarks/e93b3d71-8499-4fae-bb7c-4e540b775714?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">Adapter::Json</a></td><td><a href="http://localhost:3000/perf/the-computer/measures/latency?utm_medium=referral&utm_source=cli&utm_content=comment&utm_campaign=pr+comments&utm_term=the-computer">Latency<br/>microseconds (µs)</a></td><td>📈 <a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=e93b3d71-8499-4fae-bb7c-4e540b775714&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">plot</a><br/>🚨 <a href="https://bencher.dev/perf/bencher/alerts/91ee27a7-2aee-41fe-b037-80b786f26cd5?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">alert</a><br/>🚷 <a href="https://bencher.dev/perf/bencher/thresholds/f6ade42d-ef45-4533-b6fe-588c1f3e9405?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">threshold</a></td><td><b>3.45<br/>(+1.52%)</b></td><td><b>3.36<br/>(102.48%)</b></td></tr></tbody></table><a href="https://bencher.dev/perf/bencher/reports/36a1eeff-57f5-4b99-b058-8c9c240a9f2c?utm_medium=referral&utm_source=github&utm_content=readme&utm_campaign=readme&utm_term=bencher">🐰 View full continuous benchmarking report in Bencher</a>

## The Bencher Suite

Bencher is a suite of bare metal continuous benchmarking tools.

- [`bencher` CLI](https://bencher.dev/docs/how-to/install-cli/): run benchmarks and publish results
- Bencher API Server: store, query, and alert on results
- Bencher Console: web UI for tracking and graphing
- Bencher Bare Metal [`runner`](https://bencher.dev/docs/tutorial/bare-metal/): dedicated hardware for noise-free benchmarks

The best place to start is the [Bare Metal Quickstart](https://bencher.dev/docs/tutorial/bare-metal/).

For on-prem deployments, check out the [Bencher Self-Hosted Quickstart](https://bencher.dev/docs/tutorial/self-hosted/)

> 🐰 [Use the GitHub Action with your project](#github-actions)

## Documentation

- Tutorial
  - [Quickstart](https://bencher.dev/docs/tutorial/quickstart/)
  - [Bare Metal Quickstart](https://bencher.dev/docs/tutorial/bare-metal/)
  - [Self-Hosted Quickstart](https://bencher.dev/docs/tutorial/self-hosted/)
- How To
  - [Install CLI](https://bencher.dev/docs/how-to/install-cli/)
  - [Claim Benchmark Results](https://bencher.dev/docs/how-to/claim/)
  - [Track Benchmarks in CI](https://bencher.dev/docs/how-to/track-benchmarks/)
  - [GitHub Actions](https://bencher.dev/docs/how-to/github-actions/)
  - [GitLab CI/CD](https://bencher.dev/docs/how-to/gitlab-ci-cd/)
  - [Track Custom Benchmarks](https://bencher.dev/docs/how-to/track-custom-benchmarks/)
  - [Track Build Time](https://bencher.dev/docs/how-to/track-build-time/)
  - [Track File Size](https://bencher.dev/docs/how-to/track-file-size/)
  - [Self-Hosted GitHub App](https://bencher.dev/docs/how-to/github-app/)
  - [Self-Hosted Google OAuth](https://bencher.dev/docs/how-to/google-oauth/)
- Explanation
  - [Benchmarking Overview](https://bencher.dev/docs/explanation/benchmarking/)
  - [Bare Metal Overview](https://bencher.dev/docs/explanation/bare-metal/)
  - [`bencher run`](https://bencher.dev/docs/explanation/bencher-run/)
  - [Bare Metal Images](https://bencher.dev/docs/explanation/images/)
  - [Branches & Start Points](https://bencher.dev/docs/explanation/branches/)
  - [Testbeds & Specs](https://bencher.dev/docs/explanation/testbeds/)
  - [Thresholds & Alerts](https://bencher.dev/docs/explanation/thresholds/)
  - [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters/)
  - [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking/)
  - [Bencher Self-Hosted](https://bencher.dev/docs/explanation/bencher-self-hosted/)
  - [Bencher Talks](https://bencher.dev/docs/explanation/talks/)
- Reference
  - [REST API](https://bencher.dev/docs/api/)
  - [Bencher Metric Format](https://bencher.dev/docs/reference/bencher-metric-format/)
  - [Bencher Compose](https://bencher.dev/docs/reference/bencher-compose/)
  - [`bencher noise`](https://bencher.dev/docs/reference/bencher-noise/)
  - [Console Server Config](https://bencher.dev/docs/reference/console-config/)
  - [API Server Config](https://bencher.dev/docs/reference/server-config/)
  - [Changelog](https://bencher.dev/docs/reference/changelog/)
  - [Roadmap](https://bencher.dev/docs/reference/roadmap/)
  - [Prior Art](https://bencher.dev/docs/reference/prior-art/)
  - [Architecture](https://bencher.dev/docs/reference/architecture/)
  - [Database Schema](https://bencher.dev/docs/reference/schema/)

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
  - [Build Time (Compile Time)](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F-build-time)
  - [File Size (Binary Size)](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F-file-size)
- #️⃣ C#
  - [BenchmarkDotNet](https://bencher.dev/docs/explanation/adapters/#%EF%B8%8F%E2%83%A3-c-dotnet)
- ➕ C++
  - [Catch2](https://bencher.dev/docs/explanation/adapters/#-c-catch2)
  - [Google Benchmark](https://bencher.dev/docs/explanation/adapters/#-c-google)
- 🎯 Dart
  - [benchmark_harness](https://bencher.dev/docs/explanation/adapters/#-dart-benchmark_harness)
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
  - [Gungraun](https://bencher.dev/docs/explanation/adapters/#-rust-gungraun) (formerly [Iai-Callgrind](https://bencher.dev/docs/explanation/adapters/#-rust-iai-callgrind))
- ❯_ Shell
  - [Hyperfine](https://bencher.dev/docs/explanation/adapters/#_%EF%B8%8F-shell-hyperfine)

👉 For more details see the [explanation of benchmark harness adapters](https://bencher.dev/docs/explanation/adapters/).

Don't see your harness? [Open an issue →](https://github.com/bencherdev/bencher/issues/new?labels=adapter&title=Adapter%20request%3A%20)

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
        <a href="https://bencher.dev/perf/sedpack?branches=e27f4617-5c19-4a91-a3f5-ca006bde2dd8&heads=e0f3701a-7886-4317-bf5c-ff04e2d0ccd1&testbeds=c83cc96a-a3b8-4c8e-88d3-d86c49caa12e&benchmarks=2fed029b-b64d-40ac-9d37-e4582ac6ad6b%2C7c8dfdfe-cc70-4928-8d09-841d7864984b&measures=37d645e6-8e9a-4731-8f16-28f12c22bd1c&upper_boundary=true&end_time=1754265600000&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=3&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&start_time=1741996800000&lower_boundary=false&upper_value=false&lower_value=false&tab=branches&clear=true&branches_search=main">
          <img
            src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/google.svg"
            alt="Google"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Google Sedpack</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/git?lower_value=false&upper_value=false&lower_boundary=false&upper_boundary=true&x_axis=date_time&branches=595859eb-071c-48e9-97cf-195e0a3d6ed1&testbeds=02dcb8ad-6873-494c-aabc-9a6237601308&benchmarks=5e5c6ae1-ec8e-4c25-b27d-dcf773d33a51%2C0eb509fd-c4a8-45f3-baca-2e7e4a89b0e8&measures=63dafffb-98c4-4c27-ba43-7112cae627fc&tab=plots&plots_search=0d7f6186-f80a-4fbe-9022-75b6caf5164e&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&end_time=1745971200000&start_time=1740787200000&utm_medium=share&utm_source=bencher&utm_content=img&utm_campaign=perf%2Bimg&utm_term=git">
          <img
            src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/gitlab.svg"
            alt="GitLab"
            width="300px"
          />
        </a>
      </p>
      <p align="center">GitLab Git</p>
    </td>
  </tr>
  <tr>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/servo?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=52e1e9bb-959c-4171-a53d-e06bd694a6c1&heads=3dbe3681-11b1-4e30-b482-4ee72dc0960c&testbeds=d742c702-3842-4108-9d0c-2db74e57599a&measures=678e4118-c8a5-494d-8799-08abc3021cd5&start_time=1734048000000&end_time=1735236203000&lower_boundary=false&upper_boundary=false&clear=true&tab=benchmarks&benchmarks=c4da10d8-9539-4943-95ca-5e08df0cd6f9&benchmarks_search=servo">
          <img
            src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/servo-tlf.svg"
            alt="Servo"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Servo</p>
    </td>
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/neqo?branches=1c3aa454-5e63-4a34-bc7e-a86c397661fe&heads=a5e4e812-c619-44d3-844e-ee795a2b26e9&testbeds=f8b47e59-8dac-4a95-aec4-5bfb9756e749&measures=8bfeb966-6e8a-4719-9705-23fe985d6e40&upper_boundary=true&start_time=1762992000000&end_time=1765152000000&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&tab=benchmarks&benchmarks_search=decode+1048576+bytes&benchmarks=66a0e29f-9d91-4656-903e-d4c0c817387c%2C9c88c263-c57e-45b8-89c9-34e3a5f196cb%2C0258133c-8223-4b76-a76c-9e92a1a60f60&clear=true">
          <img
            src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/mozilla.svg"
            alt="Mozilla"
            width="300px"
          />
        </a>
      </p>
      <p align="center">Mozilla Neqo</p>
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
  </tr>
  <tr>
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
    <td>
      <p align="center">
        <a href="https://bencher.dev/perf/clap-rs-clap?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=b920383c-b9ee-4bd6-94ea-8d101b55286a&heads=5eeccfee-4fdd-405a-8554-90cd945ee1c1&testbeds=551ebdbf-b50a-4813-9064-286d2e66888f&benchmarks=b0a8ca01-4418-485e-9446-81d2a9c62774&measures=04ff075b-dc09-4c77-909a-634352fd5b02&end_time=1767052800000&lower_boundary=false&upper_boundary=false&clear=true&start_time=1748908800000&tab=branches&branches_search=master">
          <img
            src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/clap.png"
            alt="clap"
            width="300px"
          />
        </a>
      </p>
      <p align="center">clap</p>
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
      - uses: actions/checkout@v6
      - uses: bencherdev/bencher@main
      - run: bencher run "bencher mock"
```

Supported Operating Systems:
- Linux (x86_64 & ARM64)
- macOS (x86_64 & ARM64)
- Windows (x86_64 & ARM64)

<br />

👉 For more details see the [explanation of how to use GitHub Actions](https://bencher.dev/docs/how-to/github-actions/).

### Repository Secrets

Add `BENCHER_API_TOKEN` to you **Repository** secrets (ex: `Repo -> Settings -> Secrets and variables -> Actions -> New repository secret`). You can find your API tokens by running `bencher token list my-user-slug` or [view them in the Bencher Console](https://bencher.dev/console/users/tokens).

### Error on Alert

You can set the `bencher run` CLI subcommand to error
if [an Alert is generated](https://bencher.dev/docs/explanation/thresholds/) with the `--error-on-alert` flag.

```bash
bencher run --error-on-alert "bencher mock"
```

👉 For more details see the [explanation of `bencher run`](https://bencher.dev/docs/explanation/bencher-run/#--error-on-alert).

### Comment on PRs

You can set the `bencher run` CLI subcommand to comment on a PR with the `--github-actions` argument.

```bash
bencher run --github-actions "${{ secrets.GITHUB_TOKEN }}" "bencher mock"
```

👉 For more details see the [explanation of `bencher run`](https://bencher.dev/docs/explanation/bencher-run/#--github-actions/).

👉 See the [example PR comment above](#every-benchmark-run-lands-as-a-pr-comment).

### Specify CLI Version

There is also an optional `version` argument to specify an exact version of the Bencher CLI to use.
Otherwise, it will default to using the latest CLI version.

```yaml
- uses: bencherdev/bencher@main
  with:
    version: 0.6.2
```

Specify an exact version if using [Bencher _Self-Hosted_](https://bencher.dev/docs/explanation/bencher-self-hosted/).
Do **not** specify an exact version if using Bencher _Cloud_ as there are still occasional breaking changes.

## What Engineers Say

<table>
  <tr>
    <td>
      <p>Bencher is like CodeCov for performance metrics.</p>
      <br />
      <p align="center">
        <a href="https://github.com/JonathanWoollett-Light">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/JonathanWoollett-Light.jpg" width="48" height="48" alt="Jonathan Woollett-Light" />
        </a>
        <br />
        Jonathan Woollett-Light
        <br />
        <a href="https://github.com/JonathanWoollett-Light">
          @JonathanWoollett-Light
        </a>
      </p>
    </td>
    <td>
      <p>I think I'm in heaven. Now that I'm starting to see graphs of performance over time automatically from tests I'm running in CI. It's like this whole branch of errors can be caught and noticed sooner.</p>
      <br />
      <p align="center">
        <a href="https://github.com/gpwclark">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/gpwclark.jpg" width="48" height="48" alt="Price Clark" />
        </a>
        <br />
        Price Clark
        <br />
        <a href="https://github.com/gpwclark">
          @gpwclark
        </a>
      </p>
    </td>
  </tr>
  <tr>
    <td>
      <p>95% of the time I don't want to think about my benchmarks. But when I need to, Bencher ensures that I have the detailed historical record waiting there for me. It's fire-and-forget.</p>
      <br />
      <p align="center">
        <a href="https://github.com/jneem">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/jneem.jpg" width="48" height="48" alt="Joe Neeman" />
        </a>
        <br />
        Joe Neeman
        <br />
        <a href="https://github.com/jneem">
          @jneem
        </a>
      </p>
    </td>
    <td>
      <p>I've been looking for a public service like Bencher for about 10 years :)</p>
      <br />
      <p align="center">
        <a href="https://github.com/jaqx0r">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/jaqx0r.png" width="48" height="48" alt="Jamie Wilkinson" />
        </a>
        <br />
        Jamie Wilkinson
        <br />
        <a href="https://github.com/jaqx0r">
          @jaqx0r
        </a>
      </p>
    </td>
  </tr>
  <tr>
    <td>
      <p>I'm happy with how quickly I was able to get Bencher configured and working.</p>
      <br />
      <p align="center">
        <a href="https://github.com/westonpace">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/westonpace.jpg" width="48" height="48" alt="Weston Pace" />
        </a>
        <br />
        Weston Pace
        <br />
        <a href="https://github.com/westonpace">
          @westonpace
        </a>
      </p>
    </td>
    <td>
      <p>Bencher's main ideas and concepts are really well designed.</p>
      <br />
      <p align="center">
        <a href="https://github.com/freeekanayaka">
          <img src="https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/freeekanayaka.jpg" width="48" height="48" alt="Free Ekanayaka" />
        </a>
        <br />
        Free Ekanayaka
        <br />
        <a href="https://github.com/freeekanayaka">
          @freeekanayaka
        </a>
      </p>
    </td>
  </tr>
</table>

## Hosting

- **Bencher Self-Hosted**: Deploy Bencher on your own infrastructure. Bare metal, Docker, or Kubernetes. Full control, no data leaving your environment. [Deploy in 60 seconds →](https://bencher.dev/docs/tutorial/self-hosted/)
- **Bencher Cloud**: Zero infrastructure to manage. On-demand bare metal runners, billed by the minute. [Benchmark for free →](https://bencher.dev/auth/signup)

## Contributing

The easiest way to contribute is to open this repo as a [Dev Container](https://containers.dev) in [VSCode](https://code.visualstudio.com/download) by simply clicking one of the buttons below.
Everything you need will already be there!
Once set up, both the UI and API should be built, running, and seeded at [localhost:3000](http://localhost:3000) and [localhost:61016](http://localhost:61016) respectively.
To make any changes to the UI or API though, you will have to exit the startup process and restart the UI and API yourself.

For additional information on contributing, see the [Development Getting Started](DEVELOPMENT.md) guide.

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
