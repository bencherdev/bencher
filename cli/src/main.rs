#![feature(test)]

extern crate test;

use clap::{ArgEnum, Parser};

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,

    /// Benchmark tool ID
    #[clap(short, long, arg_enum, default_value = "rust_bench")]
    tool: Tool,
}

/// Supported Languages
#[derive(ArgEnum, Clone, Debug)]
enum Tool {
    /// Rust 🦀
    #[clap(name = "rust_bench")]
    RustBench,
}

fn main() {
    let args = Args::parse();

    println!("CMD: {}", args.cmd)
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
