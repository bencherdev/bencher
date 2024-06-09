// The benchmark function for `process_sample`
fn bench_process_sample(c: &mut criterion::Criterion) {
    c.bench_function("process_sample", |b| {
        // Criterion will run our benchmark multiple times
        // to try to get a statistically significant result.
        b.iter(|| {
            // Call our `process_sample` library function with a test sample.
            profiler::process_sample(profiler_common::Sample::default()).unwrap();
        })
    });
}

// Create a custom benchmarking harness named `benchmark_profiler`
criterion::criterion_main!(benchmark_profiler);
// Register our `bench_process_sample` benchmark
// with our custom `benchmark_profiler` benchmarking harness.
criterion::criterion_group!(benchmark_profiler, bench_process_sample);
