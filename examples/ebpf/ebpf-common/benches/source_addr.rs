use criterion::{criterion_group, criterion_main, Criterion};
use ebpf_common::SourceAddr;

fn bench_source_addr_v1(c: &mut Criterion) {
    c.bench_function("SourceAddr v1", |b| {
        b.iter(|| {
            for i in 0..256 {
                SourceAddr::new_v1(i);
            }
        })
    });
}

fn bench_source_addr_v2(c: &mut Criterion) {
    c.bench_function("SourceAddr v2", |b| {
        b.iter(|| {
            for i in 0..256 {
                SourceAddr::new_v2(i);
            }
        })
    });
}

fn bench_source_addr_v3(c: &mut Criterion) {
    c.bench_function("SourceAddr v3", |b| {
        b.iter(|| {
            for i in 0..256 {
                SourceAddr::new_v3(i);
            }
        })
    });
}

fn bench_source_addr_v4(c: &mut Criterion) {
    c.bench_function("SourceAddr v4", |b| {
        b.iter(|| {
            for i in 0..256 {
                SourceAddr::new_v4(i);
            }
        })
    });
}

criterion_group!(
    benches,
    bench_source_addr_v1,
    bench_source_addr_v2,
    bench_source_addr_v3,
    bench_source_addr_v4,
);
criterion_main!(benches);
