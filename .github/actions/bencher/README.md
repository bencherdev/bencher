# Bencher

This action installs the [Bencher CLI](https://bencher.dev/docs/how-to/install-cli).
Use it to [track your benchmarks](https://bencher.dev/docs/how-to/track-benchmarks) in GitHub Actions.

## Example

```yaml
name: Track benchmarks with Bencher
on: [push]
jobs:
  benchmark_with_bencher:
    name: Benchmark with Bencher
    runs-on: ubuntu-latest
    env:
      - BENCHER_API_TOKEN: ${{ secrets.BENCHER_API_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - uses: bencherdev/bencher/.github/actions/bencher@main
      - run: bencher run --project my-project-slug "bencher mock"
```

## Repository Secrets

Add `BENCHER_API_TOKEN` to you **Repository** secrets (ex: `https://github.com/my-user-slug/my-repo/settings/secrets/actions`). You can find your API tokens by running `bencher token ls --user my-user-slug` or by going to the Bencher Console (ex: `https://bencher.dev/console/users/my-user-slug/tokens`).

## Additional Documentation

- [GitHub Actions](https://bencher.dev/docs/how-to/github-actions)
- [Track Benchmarks](https://bencher.dev/docs/how-to/track-benchmarks)
- [Benchmarking Overview](https://bencher.dev/docs/explanation/benchmarking)
- [Benchmark Adapters](https://bencher.dev/docs/explanation/adapters)
- [Branch Selection](https://bencher.dev/docs/explanation/branch-selection)
- [Thresholds & Alerts](https://bencher.dev/docs/explanation/thresholds)
- [Continuous Benchmarking](https://bencher.dev/docs/explanation/continuous-benchmarking)
