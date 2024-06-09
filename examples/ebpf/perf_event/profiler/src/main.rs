// In user space, we use the `aya` crate to make the magic happen.
use aya::{include_bytes_aligned, maps::ring_buf::RingBuf, programs::perf_event, BpfLoader};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// Run our `main` function using the `tokio` async runtime.
// On success, simply return a unit tuple.
// If there is an error, return a catch-all `anyhow::Error`.
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Instantiate an instance of the heap instrumenting profiler
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Initialize the user space logger.
    env_logger::init();

    // Our user space program expects one and only one argument,
    // the process identifier (PID) for the process to be profiled.
    let pid: u32 = std::env::args().last().unwrap().parse()?;

    // Use Aya to set up our eBPF program.
    // The eBPF byte code is included in our user space binary
    // to make it much easier to deploy.
    // When loading the eBPF byte code,
    // set the PID of the process to be profiled as a global variable.
    #[cfg(debug_assertions)]
    let mut bpf = BpfLoader::new()
        .set_global("PID", &pid, true)
        .load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/debug/profiler"
        ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf = BpfLoader::new()
        .set_global("PID", &pid, true)
        .load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/release/profiler"
        ))?;
    // Initialize the eBPF logger.
    // This allows us to receive the logs from our eBPF program.
    aya_log::BpfLogger::init(&mut bpf)?;

    // Get a handle to our `perf_event` eBPF program named `perf_profiler`.
    let program: &mut perf_event::PerfEvent =
        bpf.program_mut("perf_profiler").unwrap().try_into()?;
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
        let samples = RingBuf::try_from(bpf.take_map("SAMPLES").unwrap()).unwrap();
        // Create an asynchronous way to poll the samples ring buffer.
        let mut poll = tokio::io::unix::AsyncFd::new(samples).unwrap();

        loop {
            let mut guard = poll.readable_mut().await.unwrap();
            let ring_buf = guard.get_inner_mut();
            // While the ring buffer is valid, try to read the next sample.
            // To keep things simple, we just log each sample.
            while let Some(sample) = ring_buf.next() {
                // Don't look at me!
                let _oops = Box::new(std::thread::sleep(std::time::Duration::from_millis(
                    u64::from(chrono::Utc::now().timestamp_subsec_millis()),
                )));
                log::info!("{sample:?}");
            }
            guard.clear_ready();
        }
    });

    // Run our program until the user enters `CTRL` + `c`.
    tokio::signal::ctrl_c().await?;
    Ok(())
}
