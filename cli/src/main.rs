#![feature(test)]

extern crate test;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use test::{black_box, Bencher};

    #[test]
    fn ignored() {
        assert!(true);
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let x: f64 = 211.0 * 11.0;
        let y: f64 = 301.0 * 103.0;

        b.iter(|| {
            // Inner closure, the actual test
            for _ in 1..10000 {
                black_box(x.powf(y).powf(x));
            }
        });
    }
}
