# Zero to Performance Hero: How to Benchmark and Profile Your eBPF Code in Rust

## Key Takeaways

- Kernel space eBPF code can be written in C or Rust.
User space bindings to eBPF are often written in C, C++, Rust, Go, or Python.
Using Rust for both kernel and user space code provides unmatched speed, safety, and developer experience.
- ~~Premature~~ Blind optimization is the root of all evil.
Profiling your eBPF code allows you to see where to focus your performance optimizations.
- Different profiling techniques may illuminate different areas of interest.
Use several profiling tools and triangulate on the root cause of your performance problems.
- Benchmarking allows you to measure your performance optimizations
for both your kernel and user space eBPF Rust code.
- Continuous benchmarking with tools like Bencher help prevent performance regressions before they get released.

The [silent eBPF revolution] is well underway.
From networking to observability to security,
eBPF is being used across the cloud native world to enable faster and more customizable compute.
In this article we will walk through creating a basic eBPF program in Rust.
This simple and approachable eBPF program will intentionally include a performance regression.
We will then explore using an instrumenting profiler and a sampling profiler to try to locate this performance bug.
Once we have located the bug, we will need to create benchmarks to measure our performance optimizations.
Finally, we will track our benchmarks with a Continuous Benchmarking tool to catch performance regressions as a part of continuous integration (CI).

[silent eBPF revolution]: https://www.infoq.com/articles/ebpf-cloud-native-platforms/

## Getting Started with eBPF

Extended Berkeley Packet Filter (eBPF) is a virtual machine within the Linux kernel
that executes bytecode compiled from languages like C and Rust.
eBPF allows you to extend the functionality of the kernel without having to maintain a kernel module.
The eBPF verifier ensures that your code doesn't harm the kernel by checking it at load time.
These load time checks include:
a one million instruction limit,
no unbounded loops,
and no waiting for user-space events.
Once verified, the eBPF bytecode is loaded into the eBPF virtual machine
and executed within the kernel to perform tasks like tracing syscalls,
probing user or kernel space,
instrumenting Linux Security Modules (LSM),
and filtering packets.
Initially known as Berkeley Packet Filtering (BPF), it evolved into eBPF as new use cases were added.

| Library   | User space     | eBPF   | Syscalls |
| --------- | -------------- | ------ | -------- |
| libbpf    | 🪤 C            | 🪤 C    | 🪤 C      |
| bcc       | 🐍 Python + lua | 🪤 C    | 🪤 C      |
| ebpf-go   | 🕳️ Go           | 🪤 C    | 🪤 C      |
| libbpf-rs | 🦀 Rust         | 🪤 C    | 🪤 C      |
| RedBPF    | 🦀 Rust         | 🦀 Rust | 🪤 C      |
| Aya       | 🦀 Rust         | 🦀 Rust | 🦀 Rust   |

Several languages and tool sets are available for working with eBPF.
This includes the canonical `libbpf` developed within the Linux kernel source tree.
Additional tools like `bcc` and `ebpf-go` allow user space programs to be written in Python and Go, respectively.
The Rust eBPF ecosystem includes `libbpf-rs`, RedBPF, and Aya.
Aya enables writing user space and eBPF programs in Rust without a dependency on `libbpf`.
We will be using Aya throughout the rest of this article.
Aya will allow us to leverage Rust's strengths in performance, safety, and productivity for systems programming.

## Building an eBPF Profiler

For our example, we're going to create a very basic eBPF sampling profiler.
A sampling profiler sits outside of your target application and at a set interval it samples the state of your application.
We will discuss the benefits and drawbacks of sampling profilers in depth later in this article.
For now, its just important to understand that our goal is to periodically get a snapshot of state of the target application.

We will use the `cpu-clock` counter to time our sampling frequency.
At every proverbial tick, we will run our eBPF program and take a sample.
These samples are then aggregated to tell where the target application spends the most resources.

```rust
use aya_ebpf::{maps::ring_buf::RingBuf, programs::perf_event};

// Use the Aya `perf_event` procedural macro to create an eBPF perf event.
// We take in one argument, which is the context for the perf event.
// This context is provided by the Linux kernel.
#[perf_event]
pub fn perf_profiler(ctx: PerfEventContext) -> u32 {
    // Run our `get_sample` function with the provided context.
    // If we fail to get a sample, we log that to user space.
    if get_sample(&ctx).is_none() {
        aya_log_ebpf::error!(&ctx, "Failed to run `perf_event` hook.");
    }
    // Our result is a signed 32-bit integer, which we always set to `0`.
    0
}

// Create a global variable that will be set by user space.
// It will be set to the process identifier (PID) of the profiled process.
// To do this, we have to use the `no_mangle` attribute.
// This keeps Rust from mangling the `PID` symbol so it can be properly linked.
#[no_mangle]
static PID: u32 = 0;

// Use the Aya `map` procedural macro to create a ring buffer eBPF map.
// The byte size for the ring buffer must be a power of 2 multiple of the page size.
#[map]
static PERF_EVENTS: RingBuf = RingBuf::with_byte_size(4_096 * 4_096, 0)

// Now for the fun part.
// The `get_sample` function is where we actually get the profile sample for our process.
fn get_sample(&ctx: PerfEventContext) -> Option<()> {
    // Reserve memory in the ring buffer to fit our sample.
    // If the ring buffer is full then we return early.
    let mut perf_event = PERF_EVENTS
        .reserve::<Event<SamplesEvent>>(0)?;

}

```

This Rust program uses the Aya to turn the `perf_profiler` function into an [eBPF perf event][brendan gregg perf].
That is, this code gets turned into eBPF and run inside the kernel.
To get our eBPF loaded into the kernel, we need to set things up in user space.

```rust
use aya::programs::perf_event;

// Run our `main` function using the `tokio` async runtime.
// On success, simply return a unit tuple.
// If there is an error, return a catch-all `anyhow::Error`.
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Our user space program expects one and only one argument,
    // the process identifier (PID) for the process to be profiled.
    let pid: u32 = std::env::args().last().unwrap().parse()?;

    // Use Aya to setup our eBPF program.
    // The eBPF byte code is included in our user space binary
    // to make it much easier to deploy.
    // When loading the eBPF byte code,
    // set the PID of the process to be profiled as a global variable.
    let mut ebpf = aya::EbpfLoader::new()
        .set_global("PID", &pid, true)
        .load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/release/perf_profiler"
        ))?;
    // Initialize the eBPF logger.
    // This allows us to receive logs from our eBPF program.
    aya_log::EbpfLogger::init(&mut ebpf)?;

    // Get a handle to our `perf_event` eBPF program named `perf_profiler`.
    let program: &mut perf_event::PerfEvent = ebpf
        .program_mut("perf_profiler")
        .unwrap()
        .try_into()?;
    // Load our `perf_event` eBPF program into the Linux kernel.
    program.load()?;
    // Attach to our `perf_event` eBPF program that is now running in the Linux kernel.
    program.attach(
        // We are expecting to attach to a software application.
        perf_event::PerfTypeId::Software,
        // We will use the `cpu-clock` counter to time our sampling frequency.
        perf_event::perf_sw_ids::PERF_COUNT_SW_CPU_CLOCK as u64,
        // We want to profile just a single process across any CPU.
        perf_event::PerfEventScope::OneProcessAnyCpu { pid },
        // We want to collect samples 100 times per second.
        perf_event::SamplePolicy::Frequency(100),
        // We want to profile any child processes spawned by the profiled process.
        true,
    )?;

    // Run our program until the user enters `CTL` + `c`.
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

/// This is similar to [`crate::maps::PerfEventArray`], but different in a few ways:
/// * It's shared across all CPUs, which allows a strong ordering between events.
/// * Data notifications are delivered precisely instead of being sampled for every N events; the
///   eBPF program can also control notification delivery if sampling is desired for performance
///   reasons. By default, a notification will be sent if the consumer is caught up at the time of
///   committing. The eBPF program can use the `BPF_RB_NO_WAKEUP` or `BPF_RB_FORCE_WAKEUP` flags to
///   control this behavior.
/// * On the eBPF side, it supports the reverse-commit pattern where the event can be directly
///   written into the ring without copying from a temporary location.
/// * Dropped sample notifications go to the eBPF program as the return value of `reserve`/`output`,
///   and not the userspace reader. This might require extra code to handle, but allows for more
///   flexible schemes to handle dropped samples.
///
/// To receive events you need to:
/// * Construct [`RingBuf`] using [`RingBuf::try_from`].
/// * Call [`RingBuf::next`] to poll events from the [`RingBuf`].
///
/// To receive async notifications of data availability, you may construct an
/// [`tokio::io::unix::AsyncFd`] from the [`RingBuf`]'s file descriptor and poll it for readiness.
///
/// # Minimum kernel version
///
/// The minimum kernel version required to use this feature is 5.8.


[brendan gregg perf]: https://www.brendangregg.com/perf.html


eBPF XDP programs provide efficient, custom packet handling by running before the kernel’s network stack.

eBPF XDP programs [can perform one of four actions][ebpf xdp]:
- `XDP_PASS`: Pass the packet to the network stack with optional modifications
- `XDP_DROP`: Quickly drop the packet
- `XDP_TX`: Forward the packet to the same network interface it arrived on with optional modifications
- `XDP_ABORTED`: Drop the packet due to an error in the eBPF program

We're going to keep the packet handling simple and mainly focus on the eBPF inter-process communication in our example.
Therefore, if everything goes well we will just return `XDP_PASS` with no modifications to the packet.
Otherwise, we will return `XDP_ABORTED`.

This is what our basic eBPF program looks like:

```
#[xdp(name = "fun_xdp")]
pub fn fun_xdp(ctx: XdpContext) -> u32 {
    match try_fun_xdp(&ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}
```


In the basic example, the focus is on scaffolding and interprocess communication rather than packet handling.  The initial version of the eBPF XDP application will log the IPv4 source address for each received packet.


In a basic example, the focus is on scaffolding and interprocess communication rather than packet handling, so only the XDP_PASS action is used, logging the IPv4 source address for each received packet.


[ebpf xdp]: https://prototype-kernel.readthedocs.io/en/latest/networking/XDP/implementation/xdp_actions.html

> Programmers waste enormous amounts of time thinking about, or worrying about, the speed of noncritical parts of their programs, and these attempts at efficiency actually have a strong negative impact when debugging and maintenance are considered. We should forget about small efficiencies, say about 97% of the time: pre-mature optimization is the root of all evil. Yet we should not pass up our opportunities in that critical 3%.

Donald E. Knuth

https://dl.acm.org/doi/10.1145/356635.356640

"We should forget about small efficiencies, say about 97% of the time: premature optimization is the root of all evil. Yet we should not pass up our opportunities in that critical 3%"




Instrumenting profiler
- have to compile into code DHAT

Sampling profiler
- separate form code take samples
- more performant
https://www.youtube.com/watch?v=JX0aVnpHomk





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
