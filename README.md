# 🐰 Bencher

[Bencher](https://bencher.dev) is a suite of tools designed to help catch performance regressions in CI.

It consists of:

- `bencher` CLI
- Bencher API Server
- Bencher Web UI

## Quick Start

Run:

- `docker compose up -d`

Then open your browser to [localhost](http://localhost).

## Local Build with Docker

Run:

- `docker compose -f builder.docker-compose.yml build`
- `docker compose -f local.docker-compose.yml up --build -d`

Then open your browser to [localhost](http://localhost).

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version 2.0</a>
or <a href="LICENSE-MIT">MIT license</a> at your discretion.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Bencher by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.