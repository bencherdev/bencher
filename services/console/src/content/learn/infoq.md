# Zero to Performance Hero: How to Benchmark and Profile Your eBPF Code in Rust

## Key Takeaways

- Kernel space eBPF code can be written in C or Rust.
User space bindings to eBPF are often written in C, Rust, Go, or Python.
Using Rust for both kernel and user space code provides unmatched speed, safety, and developer experience.
- ~~Premature~~ Blind optimization is the root of all evil.
Profiling your code allows you to see where to focus your performance optimizations.
- Different profiling techniques may illuminate different areas of interest.
Use several profiling tools and triangulate the root cause of your performance problems.
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
Finally, we will track our benchmarks with a [Continuous Benchmarking] tool to catch performance regressions as a part of continuous integration (CI).

[silent eBPF revolution]: https://www.infoq.com/articles/ebpf-cloud-native-platforms/
[Continuous Benchmarking]: https://bencher.dev/docs/explanation/continuous-benchmarking/

## Getting Started with eBPF

Extended Berkeley Packet Filter (eBPF) is a virtual machine within the Linux kernel
that executes bytecode compiled from languages like C and Rust.
eBPF allows you to extend the functionality of the kernel without having to develop and maintain a kernel module.
The eBPF verifier ensures that your code doesn't harm the kernel by checking it at load time.
These load time checks include:
a one million instruction limit,
no unbounded loops,
no heap allocations,
and no waiting for user space events.
Once verified, the eBPF bytecode is loaded into the eBPF virtual machine
and executed within the kernel to perform tasks like:
tracing syscalls,
probing user or kernel space,
capturing perf events,
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

eBPF programs can be written with several different programming languages and tool sets.
This includes the canonical `libbpf` developed in C within the Linux kernel source tree.
Additional tools like `bcc` and `ebpf-go` allow user space programs to be written in Python and Go, respectively.
However, they require C and `libbpf` for the eBPF side of things.
The Rust eBPF ecosystem includes `libbpf-rs`, RedBPF, and Aya.
[Aya][github aya] enables writing user space and eBPF programs in Rust without a dependency on `libbpf`.
We will be using Aya throughout the rest of this article.
Aya will allow us to leverage Rust's strengths in performance, safety, and productivity for systems programming.

[github aya]: https://github.com/aya-rs/aya

## Building an eBPF Profiler

For our example, we're going to create a very basic eBPF sampling profiler.
A sampling profiler sits outside of your target application and at a set interval it samples the state of your application.
We will discuss the benefits and drawbacks of sampling profilers in depth later in this article.
For now, it's just important to understand that our goal is to periodically get a snapshot of the stack of a target application.
Let's dive in!

First, use Aya to [set up an eBPF development environment][aya dev env].
Name your project `profiler`.
Inside of `profiler-ebpf/src/main.rs` we're going to add:

```rust
// We use the `aya_ebpf` crate to make the magic happen.
use aya_ebpf::{
    helpers::gen::{bpf_ktime_get_ns, bpf_get_stack},
    maps::ring_buf::RingBuf,
    programs::perf_event
};
use profiler_common::{Sample, SampleHeader};

// Create a global variable that will be set by user space.
// It will be set to the process identifier (PID) of the target application.
// To do this, we have to use the `no_mangle` attribute.
// This keeps Rust from mangling the `PID` symbol so it can be properly linked.
#[no_mangle]
static PID: u32 = 0;

// Use the Aya `map` procedural macro to create a ring buffer eBPF map.
// This map will be used to hold our profile samples.
// The byte size for the ring buffer must be a power of 2 multiple of the page size.
#[map]
static SAMPLES: RingBuf = RingBuf::with_byte_size(4_096 * 4_096, 0)

// Use the Aya `perf_event` procedural macro to create an eBPF perf event.
// We take in one argument, which is the context for the perf event.
// This context is provided by the Linux kernel.
#[perf_event]
pub fn perf_profiler(ctx: PerfEventContext) -> u32 {
    // Reserve memory in the ring buffer to fit our sample.
    // If the ring buffer is full then we return early.
    let mut Some(sample) = SAMPLES.reserve::<Sample>(0) else {
        aya_log_ebpf::error!(&ctx, "Failed to reserve sample.");
        return 0;
    };

    // The rest of our code is `unsafe` as we are dealing with raw pointers.
    unsafe {
        // Use the eBPF `bpf_get_stack` helper function
        // to get a user space stack trace.
        let stack_len = bpf_get_stack(
            // Provide the Linux kernel context for the tracing program.
            ctx.as_ptr(),
            // Write the stack trace to the reserved sample buffer.
            // We make sure to offset by the size of the sample header.
            sample.as_mut_ptr().byte_add(Sample::HEADER_SIZE) as *mut core::ffi::c_void,
            // The size of the reserved sample buffer alloted for the stack trace.
            Sample::STACK_SIZE,
            // Set the flag to collect a user space stack trace.
            aya_ebpf::bindings::BPF_F_USER_STACK,
        );

        // If the length of the stack trace is negative,
        // then there was an error.
        let Ok(stack_len: u64) = stack_len.try_into() else {
            aya_log_ebpf::error!(&ctx, "Failed to get stack.");
            return 0;
        }

        // Write the sample header to the reserved sample buffer.
        // This header includes important metadata about the stack trace.
        core::ptr::write_unaligned(
            sample.as_mut_ptr() as *mut core::ffi::c_void,
            SampleHeader {
                // Get the current time in nanoseconds since system boot.
                ktime: bpf_ktime_get_ns(),
                // Get the current thread group ID.
                pid: ctx.tgid(),
                // Get the current thread ID, confusingly called the `pid`.
                tid: ctx.pid(),
                // The length of the stack trace.
                // This is needed to safely read the stack trace in user space.
                stack_len
            }
        )
    }

    // Commit our sample as an entry in the ring buffer.
    // The sample will then be made visible to user space.
    sample.submit(0);

    // Our result is a signed 32-bit integer, which we always set to `0`.
    0
}

// Finally, we have to create a custom panic handler.
// This custom panic handler tells the Rust compiler that we should never panic.
// Making this guarantee is required to satisfy the eBPF verifier.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
```

This Rust program uses the Aya to turn the `perf_profiler` function into an [eBPF perf event][brendan gregg perf].
Every time this perf event is triggered, we capture a stack trace for our target application using [the `bpf_get_stack` eBPF helper function][man7 bpf_get_stack].
To get our eBPF loaded into the kernel, we need to set things up in user space.
Inside of `profiler/src/main.rs` we're going to add:

```rust
// In user space we use the `aya` crate to make the magic happen.
use aya::{maps::ring_buf::RingBuf, programs::perf_event};
use profiler_common::Sample;

// Run our `main` function using the `tokio` async runtime.
// On success, simply return a unit tuple.
// If there is an error, return a catch-all `anyhow::Error`.
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Our user space program expects one and only one argument,
    // the process identifier (PID) for the process to be profiled.
    let pid: u32 = std::env::args().last().unwrap().parse()?;

    // Use Aya to set up our eBPF program.
    // The eBPF byte code is included in our user space binary
    // to make it much easier to deploy.
    // When loading the eBPF byte code,
    // set the PID of the process to be profiled as a global variable.
    let mut ebpf = aya::EbpfLoader::new()
        .set_global("PID", &pid, true)
        .load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/release/profiler"
        ))?;
    // Initialize the eBPF logger.
    // This allows us to receive the logs from our eBPF program.
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
        // That process is the one we have the PID for.
        perf_event::PerfEventScope::OneProcessAnyCpu { pid },
        // We want to collect samples 100 times per second.
        perf_event::SamplePolicy::Frequency(100),
        // We want to profile any child processes spawned by the profiled process.
        true,
    )?;

    // Spawn a task to handle reading profile samples.
    tokio::spawn(async move {
        // Create a user space handle to our `SAMPLES` ring buffer eBPF map.
        let mut samples: RingBuf<Sample> = ebpf.map_mut("SAMPLES").try_into()?;

        // While the ring buffer is valid, try to read the next sample.
        // To keep things simple, we just log each sample.
        while let Some(sample) = samples.next() {
            // Don't look at me!
            let _oops = Box::new(tokio::time::sleep(std::time::Duration::from_millis(u64::from(chrono::Utc::now().timestamp_subsec_millis()))).await);
            tracing::info!("{sample:?}");
        }
    });

    // Run our program until the user enters `CTRL` + `c`.
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

Our user space code can now loads our perf event eBPF program.
Once loaded, our eBPF program will use the `cpu-clock` counter to time our sampling frequency.
One hundred times a second, we will sample the target application
and capture a stack trace.
This stack trace sample is then sent over to user space via the ring buffer.
Finally, the stack trace sample is printed to standard out.

This is obviously a very simple profiler.
We aren't symbolicating the call stack,
so we are just printing a list of memory addresses with some metadata.
Nor are we able to sample our target program while it is sleeping.
For that we would have to add a `sched` tracepoint for `sched_switch`.
However, this is already enough code for us to have a performance regression. Did you spot it?

[aya dev env]: https://aya-rs.dev/book/start/development/
[brendan gregg perf]: https://www.brendangregg.com/perf.html
[man7 bpf_get_stack]: https://man7.org/linux/man-pages/man7/bpf-helpers.7.html

## Profiling the Profiler

Users of our simple profiler have given us feedback that it seems to be rather sluggish.
They don't mind having to symbolicate the call stack for their sleepless programs by hand.
What really bothers them is the samples take a while to print.
Sometimes things even appear to be getting backed up.
Right about now the seemingly ubiquitous adage
"premature optimization is the root of all evil"
usually starts to get bandied around.

However, let's take a look at what Donald Knuth actually said all the way back in 1974:

> Programmers waste enormous amounts of time thinking about, or worrying about,
> the speed of noncritical parts of their programs,
> and these attempts at efficiency actually have a strong negative impact when debugging and maintenance are considered.
> We should forget about small efficiencies, say about 97% of the time: pre-mature optimization is the root of all evil.
> Yet we should not pass up our opportunities in that critical 3%.
>
> Donald E. Knuth, [Structured Programming with `go to` Statements](https://dl.acm.org/doi/10.1145/356635.356640)

So that is exactly what we need to do, look for "opportunities in that critical 3%".
In order to do so we are going to explore two different kinds of profilers, sampling and instrumenting.
We will then use each type of profiler to find that critical 3% in our own simple profiler.

Our simple eBPF profiler is an example of a sampling profiler.
It sits outside of the target application.
At a given interval, it collects a sample of the target application's stack trace.
Because a sampling profiler only runs periodically, it has relatively little overhead.
However, this means that we may miss some things.
By analogy, this is like watching a movie by only looking at one out of every one hundred frames.
Movies are usually shot at 24 frames per second.
That means you would only see a new frame about once every 4 seconds.
Besides being a very boring way to watch a film,
this can also lead to a distorted view of what is actually going on.
The frame you happen to see could really just be a momentary flashback (overweighting).
Conversely, there could have just been an amazing action sequence,
and you only caught the closeup on the lead actor's face on either side of it (underweighting).

The other major kind of profiler is an instrumenting profiler.
Unlike a sampling profiler, an instrumenting profiler is a part of the target application.
Inside of the target application, a sampling profiler collects information about the work being done.
This usually leads instrumenting profilers to have much higher overhead than sampling profilers.
Therefore a sampling profiler is more likely to give you an accurate picture
of what is going on in production than an instrumenting profiler.
To continue our analogy from above, an instrumenting profiler is like watching a movie
that was shot on an old 35mm hand cranked camera.
Being hand cranked, it was nigh impossible to consistently film at 24 frames per second.
So cinematographers settled for around 18 frames per second.
Likewise with an instrumenting profiler, you can view all of the proverbial frames,
but everything has to run much slower.
You can run right into [the observer effect][wikipedia heisenbug].

[wikipedia heisenbug]: https://en.wikipedia.org/wiki/Heisenbug

### Sampling Profiler

The go to sampling profiler on Linux is `perf`.
Under the hood, `perf` uses the exact same perf events as our own simple profiler.
There is a fantastic tool for Rust developers that wraps `perf`
and generates beautiful [flamegraphs][brendan gregg flamegraphs].
It is aptly named `flamegraph`.
Flamegraphs are a technique used to visualize stack traces created by Brendan Gregg.

To get started, follow [the `flamegraph` installation steps][github flamegraph].
Once you have `flamegraph` installed,
we can finally profile the profiler!

![flamegraph.svg]

The flamegraph that is produced is an interactive SVG file.
The length along the x-axis indicates the percentage of time that a stack was present in the samples.
This is accomplished by sorting the stacks alphabetically
and then merged identically named stacks into a single rectangle.
It is important to note that the x-axis of a flamegraph is _not_ sorted by time.
Instead it is meant to show the proportion of time used,
sort of like a mini rectangular pie chart for each row of the diagram.
The height along the y-axis indicates the stack depth, going from the bottom up.
That is, the longest lived stacks are on the bottom and newer generations are on top.
Therefore, the stack frames with a top edge exposed were the bits of code that were actively running when a sample was taken.

![peak_flamegraph.svg]

Zooming in here to this peak, we can see the call stack for our task that reads from the samples map.
We seem to be doing quite a bit of sleeping...
Now lets hop over to using an instrumenting profiler to get another vantage point.

[brendan gregg flamegraphs]: https://www.brendangregg.com/flamegraphs.html
[github flamegraph]: https://github.com/flamegraph-rs/flamegraph

### Instrumenting Profiler

There are many different things that one could measure at runtime within their application.
Some of these are idiosyncratic to the application under observation and others are more general.
For measures particular to your application, [the `counts` crate][github counts] is a useful tool.
A measure that is useful for almost all applications is heap allocations.
The easiest way to measure heap allocations in Rust
is with [the `dhat-rs` crate][github dhat rs].

To use `dhat-rs` we have to update our `profiler/src/main.rs` file:

```rust
...

// Create a custom global allocator
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Instantiate an instance of the heap instrumenting profiler
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    ...
}
```

With `dhat-heap` added as a feature
and our release builds set to keep debug symbols in our `Cargo.toml` file,
we can now run our simple profiler with the `--features dhat-heap` option.

<!-- STUB RESULTS -->
```
dhat: Total:     1,256 bytes in 6 blocks
dhat: At t-gmax: 1,256 bytes in 6 blocks
dhat: At t-end:  1,256 bytes in 6 blocks
dhat: The data has been saved to dhat-heap.json, and is viewable with dhat/dh_view.html
```

The `Total` is the total memory allocated by our simple profiler.
That is a total of 1,256 bytes in 6 allocations.
Next, `At t-gmax` indicates the largest that the heap got while running.
Finally, `At t-end` is the size of the heap at the end of our application.

As for that `dhat-heap.json`,
you can open it in [the online viewer][dh view].

![dh_view.png]

This shows you a tree structure of when and where heap allocations occurred.
The outer nodes are the parent and the inner nodes are its children.
That is, the longest lived stacks frames are on the outside and newer generations are on the inside.
Zooming in on one of those blocks, we can take a look at the allocation stack trace.

![dh_view_allocated_at.png]

Here the highest numbered field is going to be the line from our source code.
As we descend numerically, we are actually going up the stack trace.
Now spin around three times and tell [which way an icicle graph goes][polar signals].

Looking at the percentages in the DHAT viewer it seems like we are doing quite a bit of allocating...
To get a more visual representation of the DHAT results,
we can open them in [the Firefox Profiler][firefox profiler].
The Firefox Profiler also allows you to create shareable links.
This is [the link][fp link] for my DHAT profile.

At this point I think we have narrowed down the culprit:

```rust
// Don't look at me!
let _oops = Box::new(tokio::time::sleep(std::time::Duration::from_millis(u64::from(chrono::Utc::now().timestamp_subsec_millis()))).await);
```

We could probably just remove this line and call it a day.
However, let's heed the words on Donald Knuth
and really make sure we have found that critical 3%.

[github counts]: https://github.com/nnethercote/counts
[github dhat rs]: https://github.com/nnethercote/dhat-rs
[dh view]: https://nnethercote.github.io/dh_view/dh_view.html
[polar signals]: https://www.polarsignals.com/blog/posts/2023/03/28/how-to-read-icicle-and-flame-graphs
[firefox profiler]: https://profiler.firefox.com
[fp link]: https://profiler.firefox.com/tbd

## Benchmarking the Profiler

It seems like our slowdown is in the user space side of things,
so that is where we are going to focus our benchmarking efforts.
If that were not the case, we would have to [build a custom eBPF benchmarking harness][thenewstack ebpf benchmark].
Lucky for us, we can use a less bespoke solution to test our user space source code.

We will need to refactor our `profiler/src/main.rs` file.
Benchmarks in Rust can only be run against libraries and not binaries.
Thus, we have to create a new `profiler_common/src/lib.rs` file
that will be used by both our binary and our benchmarks.

Refactoring our code to break out our sample processing logic,
gives us this library function:

```rust
pub async fn process_sample(sample: Sample) -> Result<(), anyhow::Error> {
    // Don't look at me!
    let _oops = Box::new(tokio::time::sleep(std::time::Duration::from_millis(u64::from(chrono::Utc::now().timestamp_subsec_millis()))).await);
    tracing::info!("{sample:?}");

    Ok(())
}
```

Next we are going to add benchmarks using [Criterion][github criterion].
After adding Criterion as our benchmarking harness in our `Cargo.toml`,
we can create a benchmark for our `process_sample` library function.

```rust
// The benchmark function for `process_sample`
fn bench_process_sample(c: &mut criterion::Criterion) {
    c.bench_function("process_sample", |b| {
        // Criterion will run our benchmark multiple times
        // to try to get a statistically significant result.
        b.iter(|| {
            // Iterate through a fixed set of test samples.
            for sample in TEST_SAMPLES {
                // Call our `process_sample` library function.
                profiler_common::process_sample(sample).unwrap();
            }
        })
    });
}

// Create a custom benchmarking harness named `benchmark_profiler`
criterion::criterion_main!(benchmark_profiler);
// Register our `bench_process_sample` benchmark
// with our custom `benchmark_profiler` benchmarking harness.
criterion::criterion_group!(benchmark_profiler, bench_process_sample);
```

When we run our benchmark with `cargo bench`
we get a result that looks something like this:

<!-- STUB RESULTS -->
```
     Running benches/adapter.rs (/Users/epompeii/Code/bencher/target/release/deps/adapter-386b3ef4962988a8)
Gnuplot not found, using plotters backend
Benchmarking process_sample: Collecting 100 samples in estimated 5.0
process_sample   time:   [3.3547 µs 3.3705 µs 3.3864 µs]
Found 4 outliers among 100 measurements (4.00%)
  3 (3.00%) low mild
  1 (1.00%) high mild
```

Now lets remove that pesky `oops` line from `process_sample` and see what happens:

<!-- STUB RESULTS -->
```
     Running benches/adapter.rs (/Users/epompeii/Code/bencher/target/release/deps/adapter-865fae6b02d66e20)
Gnuplot not found, using plotters backend
Benchmarking process_sample: Collecting 100 samples in estim
process_sample          time:   [2.4256 µs 2.4402 µs 2.4563 µs]
                        change: [-2.7353% -1.8949% -1.0559%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 4 outliers among 100 measurements (4.00%)
  3 (3.00%) high mild
  1 (1.00%) high severe
```

Excellent!
Criterion is able to compare the results between our local runs
and let us know that our performance has improved.
You can also dig deeper into [how to benchmark Rust code with Criterion][bencher criterion],
if you're interested in a step-by-step guide.
Going the other way, if we now add that `oops` line back,
Criterion will let us know that we have a performance regression.

<!-- STUB RESULTS -->
```
     Running benches/adapter.rs (/Users/epompeii/Code/bencher/target/release/deps/adapter-865fae6b02d66e20)
Gnuplot not found, using plotters backend
Benchmarking process_sample: Collecting 100 samples in estim
process_sample          time:   [3.1768 µs 3.1976 µs 3.2226 µs]
                        change: [+3.3789% +5.3157% +7.5451%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 8 outliers among 100 measurements (8.00%)
  4 (4.00%) high mild
  4 (4.00%) high severe
```

It's tempting to call this a job well done.
We have found and fixed our opportunity in that critical 3%.
However, what's preventing us from introducing another performance regression just like `oops` in the future?
For most software projects the answer to that is surprisingly, "Nothing."
This is where Continuous Benchmarking comes in.

[thenewstack ebpf benchmark]: https://thenewstack.io/catch-performance-regressions-benchmark-ebpf-program/
[github criterion]: https://github.com/bheisler/criterion.rs
[bencher criterion]: https://bencher.dev/learn/benchmarking/rust/criterion/

## Continuous Benchmarking

Continuous Benchmarking is a software development practice of frequent,
automated benchmarking to quickly detect performance regressions.
This reduces the cycle time for detecting performance regressions
from days and weeks to hours and minutes.
For the same reasons that unit tests are run as a part of Continuous Integration for each code change,
benchmarks should be run as a part of Continuous Benchmarking for each code change.

To add Continuous Benchmarking to our simple profiler,
we're going to use [Bencher][bencher].
Bencher is an [open source Continuous Benchmarking tool][github bencher].
This means you can easily self-host Bencher.
However, for this tutorial we are going to use a free account on Bencher Cloud.
Go ahead and [signup for a free account][bencher signup].
Once you are logged in, new user onboarding should provide you with an API token
and ask you to name your first project.
Name your project `Simple Profiler`.
Next, follow the instructions to [install the `bencher` CLI][bencher cli].
With the `bencher` CLI installed, we can now start tracking our benchmarks.

```
bencher run \
    --project simple-profiler \
    --token $BENCHER_API_TOKEN \
    cargo bench
```

This command uses the `bencher` CLI to run `cargo bench` for us.
The `bencher run` command parses the results of `cargo bench`
and sends them to the Bencher API server under our `Simple Profiler` project.
Click on the link in the CLI output to view a plot of your first results.
After running our benchmarks a few times, my perf page looks like this:

<!-- STUB RESULTS -->
<iframe src="https://bencher.dev/perf/rustls-821705769/embed?key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=branches&measures=013468de-9c37-4605-b363-aebbbf63268d&branches=28fae530-2b53-4482-acd4-47e16030d54f&testbeds=62ed31c3-8a58-479c-b828-52521ed67bee&benchmarks=bd25f73c-b2b9-4188-91b4-f632287c0a1b%2C8d443816-7a23-40a1-a54c-59de911eb517%2C42edb37f-ca91-4984-8835-445514575c85&start_time=1704067200000" title="rustls" width="100%" height="780px" allow="fullscreen"></iframe>

By saving our results to Bencher,
we can now track and compare our results over time
and across several different dimensions.
Bench supports tracking results based on the:

- Branch: The `git` branch used (ex: `main`)
- Testbed: The testing environment (ex: `ubuntu-latest` for GitHub Actions)
- Benchmark: The performance test that was run (ex: `process_sample`)
- Measure: The unit of measure for the benchmark (ex: `latency` in nanoseconds)

Now that we have benchmark tracking in place,
it is time to take care of the "continuous" part.
There are step-by-step guides for Continuous Benchmarking
in [GitHub Actions][bencher github actions] and [GitLab CI/CD][bencher gitlab ci cd] available.
For our example though, we're going to implement Continuous Benchmarking
without worrying about the specific CI provider.

We will have two different CI jobs.
One to track our default `main` branch,
and another to catch performance regressions in candidate branches
(pull requests, merge requests, etc).

For our `main` branch job, we'll have a command like this:

```
bencher run \
    --project simple-profiler \
    --token $BENCHER_API_TOKEN \
    --branch main \
    --testbed ci-runner \
    --err \
    cargo bench
```

For clarity, we explicitly set our `branch` as `main`.
We also set our `testbed` to a name for the CI runner, `ci-runner`.
Finally, we set things to fail if we generate an alert
with the `--err` flag.

For the candidate branch, we'll have a command like this:

```
bencher run \
    --project simple-profiler \
    --token $BENCHER_API_TOKEN \
    --branch $CANDIDATE_BRANCH \
    --branch-start-point $DEFAULT_BRANCH \
    --branch-start-point-hash $DEFAULT_BRANCH_HASH \
    --testbed ci-runner \
    --err \
    bencher mock
```

Here things get a little more complicated.
Since we want our candidate branch to be compared to our default branch,
we are going to need to use some environment variables provided by our CI system.

- `$CANDIDATE_BRANCH` should be the candidate branch name
- `$DEFAULT_BRANCH` should be the default branch name (ie: `main`)
- `$DEFAULT_BRANCH_HASH` should be the current default branch `git` hash

For a more detailed guide, see [how to track benchmarks in CI][bencher track benchmarks]
for a step-by-step walk through.

With Continuous Benchmarking in place,
we can now iterate on on your simple profiler
without worrying about introducing performance regressions into our code.
Continuous Benchmarking is not meant to replace profiling or running benchmarks locally.
It is meant to complement them.
Analogously, continuous integration has not replaced using a debugger or running unit tests locally.
It has complemented them by providing a backstop for feature regressions.
In this same vein, Continuous Benchmarking provides a backstop for preventing performance regressions before they make it to production.

[bencher]: https://bencher.dev
[github bencher]: https://github.com/bencherdev/bencher
[bencher signup]: https://bencher.dev/auth/signup
[bencher cli]: https://bencher.dev/docs/how-to/install-cli/
[bencher github actions]: https://bencher.dev/docs/how-to/github-actions/
[bencher gitlab ci cd]: https://bencher.dev/docs/how-to/gitlab-ci-cd/
[bencher track benchmarks]: https://bencher.dev/docs/how-to/track-benchmarks/

## Wrap Up

eBPF is a powerful tool that allows software engineers to add custom capabilities to the Linux kernel,
without having to be a kernel developer.
We surveyed the existing options for creating eBPF programs.
Based on the requirements of speed, safety, and developer experience,
we chose to build our sample program in Rust using Aya.

The simple profiler that we built contained a performance regression.
Following the wisdom of Donald Knuth, we set out to discover what critical 3% of our simple profiler we needed to fix.
We triangulated our performance regressions by using
a sampling profiler based on `perf` that was visualized with flamegraphs
and an instrumenting profiler for heap allocations based on DHAT.

With our performance regression pinpointed,
we then set about verifying our fix with a benchmark.
The Criterion benchmarking harness proved invaluable for local benchmarking.
However, to prevent performance regressions before they get merged we implemented Continuous Benchmarking.
Using Bencher, we were able to set up Continuous Benchmarking to catch performance regressions in CI.
