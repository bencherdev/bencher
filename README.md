# 🐰 Bencher

[Bencher](https://bencher.dev) is a suite of [continuous benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking) tools designed to help catch performance regressions in CI. That is, Bencher allows you to detect and prevent performance regressions _before_ they make it to production.

Bencher consists of:

- `bencher` CLI
- Bencher API Server
- Bencher Web UI

The best place to start is the [Bencher Quick Start](https://bencher.dev/docs/tutorial/quick-start) tutorial.

Though Bencher is open source, there is also a hosted version available [Bencher Cloud](https://bencher.dev).

## Documentation

- Tutorial
  - [Quick Start](https://bencher.dev/docs/tutorial/quick-start)
- How To
  - [Install CLI](https://bencher.dev/docs/how-to/install-cli)
  - [GitHub Actions](https://bencher.dev/docs/how-to/github-actions)
  - [GitLab CI/CD](https://bencher.dev/docs/how-to/gitlab-ci-cd)
- Explanation
  - [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking)
  - [CLI Adapters](https://bencher.dev/docs/explanation/cli-adapters)
  - [CLI Branch Selection](https://bencher.dev/docs/explanation/cli-branch-selection)
  - [Talks](https://bencher.dev/docs/explanation/talks)
- Reference
  - [REST API](https://bencher.dev/docs/reference/api)
  - [Server Config](https://bencher.dev/docs/reference/server-config)
  - [Prior Art](https://bencher.dev/docs/reference/prior-art)
  - [Roadmap](https://bencher.dev/docs/reference/roadmap)
  - [Changelog](https://bencher.dev/docs/reference/changelog)

## Supported Benchmark Harnesses

- Rust
  - [libtest bench](https://doc.rust-lang.org/rustc/tests/index.html#benchmarks)
  - [Criterion](https://docs.rs/criterion/latest/criterion/)

For more details see the [explanation of CLI adapters](https://bencher.dev/docs/explanation/cli-adapters).

## Contributing

The easiest way to contribute is to open the repo in GitPod.
Everything you need will already be there!
It is best to connect to the GitPod instance with VS Code Desktop via SSH.
There is a hotkey set to tapping caps lock twice to prompt the web VS Code to create a VS Code Desktop SSH session.
For more details see the [GitPod docs here](https://www.gitpod.io/docs/references/ides-and-editors/vscode).
Once set up, both the UI and API should be built, running, and seeded at [localhost:3000](http://localhost:3000) and [localhost:61016](http://localhost:61016) respectively.

[![Open in Gitpod](https://gitpod.io/button/open-in-gitpod.svg)](https://gitpod.io/#https://github.com/bencherdev/bencher)

## License
All content that resides under any directory named "plus" is licensed under the <a href="LICENSE-PLUS">Bencher Plus License</a>.

All other content is license under either of <a href="LICENSE-APACHE">Apache License, Version 2.0</a>
or <a href="LICENSE-MIT">MIT license</a> at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.