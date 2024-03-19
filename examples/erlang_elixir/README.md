# Erlang and Elixir benchmarks

This repository contains benchmarks for [Erlang](https://www.erlang.org) and [Elixir](https://elixir-lang.org). To install these languages, please follow the guides on at https://elixir-lang.org/install.html.

Before running benchmarks, you need to install the required dependencies with:

```bash
mix deps.get
```

## Erlang

Run Erlang benchmarks using the [erlperf](https://hexdocs.pm/erlperf/readme.html) library:

```bash
mix run lib/benchmark/erlperf.exs
```

## Elixir

Run Elixir benchmarks using the [benchee](https://hexdocs.pm/benchee/readme.html) library:

```bash
mix run lib/benchmark/benchee.exs
```
