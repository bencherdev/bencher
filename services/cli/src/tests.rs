#![cfg(test)]
use test::{
    black_box,
    Bencher,
};

#[test]
fn ignored() {
    assert!(true);
}

fn benchmark(x: f64, y: f64) {
    for _ in 1..10000 {
        black_box(x.powf(y).powf(x));
    }
}

#[bench]
fn benchmark_a(b: &mut Bencher) {
    b.iter(|| {
        // Inner closure, the actual test
        benchmark(2000.0, 30000.0);
    });
}

#[bench]
fn benchmark_b(b: &mut Bencher) {
    b.iter(|| {
        // Inner closure, the actual test
        benchmark(4000.0, 60000.0);
    });
}

#[bench]
fn benchmark_c(b: &mut Bencher) {
    b.iter(|| {
        // Inner closure, the actual test
        benchmark(8000.0, 100000.0);
    });
}
