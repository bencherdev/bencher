// In eBPF, we can’t use the Rust standard library.
#![no_std]
// The kernel calls our `perf_event`, so there is no `main` function.
#![no_main]

// We use the `aya_ebpf` crate to make the magic happen.
use aya_ebpf::{
    helpers::gen::{bpf_get_stack, bpf_ktime_get_ns},
    macros::{map, perf_event},
    maps::ring_buf::RingBuf,
    programs::PerfEventContext,
    EbpfContext,
};
use profiler_common::{Sample, SampleHeader};

// Create a global variable that will be set by user space.
// It will be set to the process identifier (PID) of the target application.
// To do this, we must use the `no_mangle` attribute.
// This keeps Rust from mangling the `PID` symbol so it can be properly linked.
#[no_mangle]
static PID: u32 = 0;

// Use the Aya `map` procedural macro to create a ring buffer eBPF map.
// This map will be used to hold our profile samples.
// The byte size for the ring buffer must be a power of 2 multiple of the page size.
#[map]
static SAMPLES: RingBuf = RingBuf::with_byte_size(4_096 * 4_096, 0);

// Use the Aya `perf_event` procedural macro to create an eBPF perf event.
// We take in one argument, the context for the perf event.
// This context is provided by the Linux kernel.
#[perf_event]
pub fn perf_profiler(ctx: PerfEventContext) -> u32 {
    // Reserve memory in the ring buffer to fit our sample.
    // If the ring buffer is full, then we will return early.
    let Some(mut sample) = SAMPLES.reserve::<Sample>(0) else {
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
            sample.as_mut_ptr().byte_add(SampleHeader::SIZE) as *mut core::ffi::c_void,
            // The size of the reserved sample buffer allocated for the stack trace.
            Sample::STACK_SIZE as u32,
            // Set the flag to collect a user space stack trace.
            aya_ebpf::bindings::BPF_F_USER_STACK as u64,
        );

        // If the length of the stack trace is negative, then there was an error.
        let Ok(stack_len) = u64::try_from(stack_len) else {
            aya_log_ebpf::error!(&ctx, "Failed to get stack.");
            // If there was an error, discard the sample.
            sample.discard(aya_ebpf::bindings::BPF_RB_NO_WAKEUP as u64);
            return 0;
        };

        // Write the sample header to the reserved sample buffer.
        // This header includes important metadata about the stack trace.
        core::ptr::write_unaligned(
            sample.as_mut_ptr() as *mut SampleHeader,
            SampleHeader {
                // Get the current time in nanoseconds since system boot.
                ktime: bpf_ktime_get_ns(),
                // Get the current thread group ID.
                pid: ctx.tgid(),
                // Get the current thread ID, confusingly called the `pid`.
                tid: ctx.pid(),
                // The length of the stack trace.
                // This is needed to safely read the stack trace in user space.
                stack_len,
            },
        )
    }

    // Commit our sample as an entry in the ring buffer.
    // The sample will then be made visible to the user space.
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
