## Benchmark
- Rust
  - The Ultimate Guide on how to Benchmark Rust Code
- C++
  - Google
  - Catch2
- Python
  - pytest
  - asv
- Go
  - go
- Shell
  - hyperfine
- C#
  - DotNet
- Java
  - JMH
- Ruby
  - Benchmark

## Benchmarking
- https://www.researchgate.net/publication/334047447_Pro_NET_Benchmarking_The_Art_of_Performance_Measurement

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

## Case Study
- Diesel

## Engineering
- Issue Bounty Programs: The Cobra Effect for Maintainer Burnout?
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
- What to do with benchmarking outliers
  - https://github.com/dotnet/BenchmarkDotNet/issues/1256#issuecomment-538746032
- Database profiling
  - https://www.youtube.com/watch?v=lDoEqZOCdM0&t=444s

## Tools
- Assembly viewer
  - https://godbolt.org/
  - https://cppinsights.io/