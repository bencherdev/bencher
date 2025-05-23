# Docs
- How To:
  - How to Import Historical Data
    - Create github-action-benchmark importer `bencher import`
- Explanation:
  - How It Works
  - Passwordless Auth

# Learn

## Benchmark
- Rust
  - The Ultimate Guide on how to Benchmark Rust Code
  - How to track Rust benchmarks in CI
  - How to profile executable size in Rust
    - [cargo-bloat](https://github.com/RazrFalcon/cargo-bloat)
    - https://github.com/rust-lang/backtrace-rs/pull/542
  - How to track compile times in CI
- C++
  - Catch2
  - How to track C++ benchmarks in CI
  - How to profile executable size in C++
    - https://github.com/google/bloaty
  - How to track compile times in CI
- Python
  - pytest
  - asv
  - How to track Python benchmarks in CI
  - How to track build times in CI
- Go
  - go
  - How to track Go benchmarks in CI
  - How to track compile times in CI
- Shell
  - hyperfine
  - How to track command line applications benchmarks in CI
- C#
  - DotNet
  - How to track C# benchmarks in CI
  - How to profile executable size on Windows
    - https://devblogs.microsoft.com/performance-diagnostics/sizebench-a-new-tool-for-analyzing-windows-binary-size/
  - How to track compile times in CI
- Java
  - JMH
  - How to track Java benchmarks in CI
  - How to track compile times in CI
- Javascript
  - How to track build times in CI
- Ruby
  - Benchmark
  - How to track Ruby benchmarks in CI
  - How to track build times in CI

## Benchmarking
- https://www.researchgate.net/publication/334047447_Pro_NET_Benchmarking_The_Art_of_Performance_Measurement
- https://blog.nelhage.com/post/reflections-on-performance/

- Why use a black box when benchmarking?
  - I'm confused about the part where you use the black box for the benchmark function. Is there a reason to profile unoptimized code?

## Keywords from users
- benchmarks history
- upload benchmarks
- chart benchmarks results
- save benchmark results for every commit and have a dashboard to visualize them over time
- "benchmark data visualization tools" and "software benchmark tooling".

## Continuous Benchmarking
- The Ultimate Guide to Continuous Benchmarking
    - https://launchdarkly.com/the-definitive-guide-to-feature-management/
    - What is Continuous Benchmarking?
    - Run
      - Benchmark
        - https://martinfowler.com/articles/practical-test-pyramid.html
        - Micro Benchmarks
        - Macro Benchmarks
      - Bare metal server
    - Track
        - Which summary statistics to collect and compare
    - Catch
        - Which statistical analysis to apply
    - Profile
        - Debugging feature regressions
    - Bencher
        - All in continuous benchmarking suite

# Profile
- Profiled guided optimization:
  - https://doc.rust-lang.org/rustc/profile-guided-optimization.html
  - https://blog.rust-lang.org/inside-rust/2020/11/11/exploring-pgo-for-the-rust-compiler.html
  - https://github.com/Kobzol/cargo-pgo
  - https://en.wikipedia.org/wiki/Profile-guided_optimization
- How to use perf
  - https://www.youtube.com/watch?v=nXaxk27zwlk
- Building your own eBPF based profiler
- https://www.martinfowler.com/ieeeSoftware/yetOptimization.pdf

# Machine Learning
- https://harvard-edge.github.io/cs249r_book/contents/benchmarking/benchmarking.html
- https://www.neuraldesigner.com/blog/how-to-benchmark-the-performance-of-machine-learning-platforms/

## Case Study
- Rustc Perf

Intention + Obstacle

## Engineering
- Engineering Bets Scorecard
  - https://mcfunley.com/choose-boring-technology
  - Rust
    - Dropshot
      - Proginator
    - Oso
  - Litestream
    - Killed replication
    - LiteFS
    - SQLite DX is amazing
  - Typescript
    - Solidjs
      - SolidStart
    - Astro
      - Things break a lot
- Issue Bounty Programs: The Cobra Effect for Maintainer Burnout?
- Escaping down the stack: Why I choose Rust in the age of LLMs
- CPU Caches:
  - https://www.youtube.com/watch?v=WDIkqP4JbkE
  - https://people.freebsd.org/~lstewart/articles/cpumemory.pdf
  - https://en.wikipedia.org/wiki/Cache-oblivious_algorithm
- Instruction counts vs wall clock time
  - https://blog.rust-lang.org/inside-rust/2020/11/11/exploring-pgo-for-the-rust-compiler.html
- Observer effect in benchmarking
- Benchmarking is hard
  - https://www.youtube.com/watch?v=zWxSZcpeS8Q
- Bessel's Correction
  - https://en.wikipedia.org/wiki/Bessel%27s_correction
- Configuring system for benchmarking
  - https://llvm.org/docs/Benchmarking.html
  - https://pyperf.readthedocs.io/en/latest/system.html
  - https://github.com/softdevteam/krun
  - https://www.mongodb.com/blog/post/reducing-variability-performance-tests-ec2-setup-key-results
- What to do with benchmarking outliers
  - https://github.com/dotnet/BenchmarkDotNet/issues/1256#issuecomment-538746032
- Database profiling
  - https://www.youtube.com/watch?v=lDoEqZOCdM0&t=444s
- Change point detection
  - https://github.com/mongodb/signal-processing-algorithms
- Benchmarketing
- DeWitt clause
  - https://www.brentozar.com/archive/2018/05/the-dewitt-clause-why-you-rarely-see-database-benchmarks/

## Tools
- Assembly viewer
  - https://godbolt.org/
  - https://cppinsights.io/
- gnuplot
  - http://www.gnuplot.info/
- Plotters
  - https://github.com/plotters-rs/plotters

## Programming
- Create error message help pages for each supported programming language
  - https://kinsta.com/knowledgebase/dns-server-not-responding/

# Binary Size Profiling

permissionless pilot
https://techcrunch.com/2023/09/05/create-a-permissionless-pilot-program-that-drives-sales-and-delights-customers/
https://www.emergetools.com/explore
- https://github.com/RazrFalcon/cargo-bloat
- https://github.com/google/bloaty
- https://github.com/rustwasm/twiggy
- https://www.emergetools.com/app/example/ios/wikipedia
- https://linux.die.net/man/1/pahole
- https://linux.die.net/man/1/readelf

Or instead of binary size could do compile times?

# Daily Code Games

Think Advent of Code meets Wordle
A daily Shenzhen I/O style puzzle maybe using a more practical instruction set, something like RISC-V or WASM.

- https://en.m.wikipedia.org/wiki/Shenzhen_I/O
- https://en.wikipedia.org/wiki/RISC-V

Could also lend itself well to having documentation on instruction set itself.

There could be a leader board. The way that it works would be pretty simple.
It would use tango style pairwise benchmarking to compare the solutions.
So basically, you could just implement a sort algorithm where the comparison function is the pairwise benchmark results.
If WASM is chosen, then it should be pretty simple to safely run the code.

Factorio style daily challenges?

Or just an advent of code leader board: https://codspeed.io/advent
