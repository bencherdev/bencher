#![feature(test)]

use std::process::Command;

extern crate test;

use clap::{ArgEnum, Parser};

const WINDOWS_SHELL: &str = "cmd";
const WINDOWS_FLAG: &str = "/C";
const UNIX_SHELL: &str = "/bin/sh";
const UNIX_FLAG: &str = "-c";

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,

    /// Shell command path
    #[clap(short, long)]
    shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
    flag: Option<String>,

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

    println!("CMD: {}", args.cmd);

    let shell = if let Some(shell) = args.shell {
        shell
    } else if cfg!(target_family = "windows") {
        WINDOWS_SHELL.into()
    } else if cfg!(target_family = "unix") {
        UNIX_SHELL.into()
    } else {
        return;
    };

    let flag = if let Some(flag) = args.flag {
        flag
    } else if cfg!(target_family = "windows") {
        WINDOWS_FLAG.into()
    } else if cfg!(target_family = "unix") {
        UNIX_FLAG.into()
    } else {
        return;
    };

    let output = Command::new(shell).arg(flag).arg(args.cmd).output();

    println!("{:?}", output);

    match args.tool {
        Tool::RustBench => {}
    }
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
