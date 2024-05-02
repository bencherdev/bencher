# Zero to Performance Hero: How to Benchmark and Profile Your eBPF Code in Rust

## Key Takeaways

- Kernel space eBPF code can be written in C or Rust.
User space bindings to eBPF are often written in C, Rust, Go, or Python.
Using Rust for both kernel and user space code provides unmatched speed, safety, and developer experience.
- 





The [silent eBPF revolution] well underway.
From networking to observability to security,
eBPF is being used across the cloud native world to enable faster and more customizable compute.


[silent eBPF revolution]: https://www.infoq.com/articles/ebpf-cloud-native-platforms/


> Programmers waste enormous amounts of time thinking about, or worrying about, the speed of noncritical parts of their programs, and these attempts at efficiency actually have a strong negative impact when debugging and maintenance are considered. We should forget about small efficiencies, say about 97% of the time: pre-mature optimization is the root of all evil. Yet we should not pass up our opportunities in that critical 3%.

Donald E. Knuth

https://dl.acm.org/doi/10.1145/356635.356640

"We should forget about small efficiencies, say about 97% of the time: premature optimization is the root of all evil. Yet we should not pass up our opportunities in that critical 3%"


There is no doubt that the grail of efficiency leads to abuse. Programmers waste enormous amounts of time thinking about, or worrying about, the speed of noncritical parts of their programs, and these attempts at efficiency actually have a strong negative impact when debugging and maintenance are considered. We should forget about small efficiencies, say about 97% of the time: pre-mature optimization is the root of all evil.
Yet we should not pass up our opportunities in that critical 3%. A good programmer will not be lulled into complacency by such reasoning, he will be wise to look carefully at the critical code; but only after that code has been identified. It is often a mistake to make a priori judgments about what parts of a program are really critical, since the universal experience of programmers who have been using measurement tools has been that their intuitive guesses fail.


Make sure to include the 5 key takeaways at the beginning of the article.

1. Write a basic eBPF program in Rust
2. Profile the source code
3. Benchmark the user space Rust code
4. Benchmark the kernel space eBPF code
5. Catch performance regressions in CI

The target reader for the article

A mid to senior level developer with an interest in eBPF and cursory knowledge of Rust.
They desire to understand how to gauge the performance impact of their eBPF code before deploying to
production.

How is this proposed article different and unique from other articles already published on the same
topic? Please provide specific use case information and technical details to help better assess the
proposal.

There are no existing articles that cover the profiling and benchmarking eBPF code written in Rust, for
both user space and kernel space.
The addition of continuous benchmarking to catch performance regressions in CI is a further
differentiator.

Technologies and tools discussed in the article

- eBPF
- Rust (language)
- Aya (Rust eBPF framework)
- DHAT (heap profiling)
- perf (profiling)
- flamegraph (visualizer perf output)
- cargo (for Rust)
- Bencher (for continuous benchmarking)

Any case studies and use cases you cover in the article?

The code example (see below) will be used to illustrate the use cases:
- profiling and catching a performance regression
- benchmarking to validate fixing the performance regression
- continuous benchmarking to prevent any future performance regressions

Are there code examples you will include?

Yes, there will be a simple and approachable Rust program that intentionally includes a performance
regression. The profiling tools will be used to detect this regression. Then a custom benchmarking
harness will be constructed to validate fixing the performance regression. Finally the custom
benchmarking harness will be hooked up to continuous benchmarking to prevent any future performance
regressions.

Five key takeaways of the article. This is the most relevant information in the article
summarized in 5 complete sentences.
Define specific takeaways from the article. A reader of your article should be able to walk away with a
set of actions to perform, a new theory to think about, or a thought-provoking question to answer.

1. Building an eBPF program in Rust is very approachable using Aya.
2. DHAT heap based profiling is easy to add to your user space code.
3. The flamegraph CLI is a very developer friendly way to visualize the profile of your user space code.
4. A custom benchmarking harness can be used to track the performance of eBPF kernel code.
5. Continuous benchmarking with tools like Bencher help prevent performance regressions
