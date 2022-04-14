#![feature(test)]

extern crate test;

mod adapter;
mod args;
mod cli;
mod error;
mod report;

use crate::cli::Cli;
use crate::error::CliError;

fn main() -> Result<(), CliError> {
    let cli = Cli::new()?;
    let output = cli.benchmark()?;
    let report = cli.convert(output)?;

    // TODO this should be the JSON value
    println!("{report:?}");

    Ok(())
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
