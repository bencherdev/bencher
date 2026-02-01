//! Bencher Jailer CLI.
//!
//! Usage:
//!   bencher-jailer --id <ID> --exec <PATH> --jail-root <PATH> [OPTIONS] [-- ARGS...]
//!
//! Or with a config file:
//!   bencher-jailer --config <PATH>

#![cfg(feature = "plus")]
#![cfg_attr(not(target_os = "linux"), allow(unused_crate_dependencies))]
#![expect(clippy::print_stderr)]

use std::process::ExitCode;

#[cfg(target_os = "linux")]
use std::env;

#[cfg(target_os = "linux")]
use bencher_jailer::{Jail, JailConfig, JailerError};
#[cfg(target_os = "linux")]
use camino::Utf8PathBuf;

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        match run() {
            Ok(exit_code) => {
                if exit_code == 0 {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(exit_code as u8)
                }
            }
            Err(e) => {
                eprintln!("bencher-jailer: {e}");
                ExitCode::FAILURE
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("bencher-jailer: requires Linux");
        ExitCode::FAILURE
    }
}

#[cfg(target_os = "linux")]
fn run() -> Result<i32, JailerError> {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let config = parse_args(&args)?;

    // Create and run jail
    let mut jail = Jail::new(config)?;
    jail.run()
}

#[cfg(target_os = "linux")]
fn parse_args(args: &[String]) -> Result<JailConfig, JailerError> {
    let mut id: Option<String> = None;
    let mut exec_path: Option<Utf8PathBuf> = None;
    let mut jail_root: Option<Utf8PathBuf> = None;
    let mut config_file: Option<String> = None;
    let mut uid: Option<u32> = None;
    let mut gid: Option<u32> = None;
    let mut memory: Option<u64> = None;
    let mut cpu: Option<f64> = None;
    let mut exec_args: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                config_file = args.get(i).cloned();
            }
            "--id" => {
                i += 1;
                id = args.get(i).cloned();
            }
            "--exec" => {
                i += 1;
                exec_path = args.get(i).map(|s| Utf8PathBuf::from(s.as_str()));
            }
            "--jail-root" => {
                i += 1;
                jail_root = args.get(i).map(|s| Utf8PathBuf::from(s.as_str()));
            }
            "--uid" => {
                i += 1;
                uid = args.get(i).and_then(|s| s.parse().ok());
            }
            "--gid" => {
                i += 1;
                gid = args.get(i).and_then(|s| s.parse().ok());
            }
            "--memory" => {
                i += 1;
                memory = args.get(i).and_then(|s| parse_size(s));
            }
            "--cpu" => {
                i += 1;
                cpu = args.get(i).and_then(|s| s.parse().ok());
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--" => {
                // Everything after -- is exec args
                exec_args = args[i + 1..].to_vec();
                break;
            }
            arg if arg.starts_with('-') => {
                return Err(JailerError::Config(format!("unknown option: {arg}")));
            }
            _ => {
                // Positional argument - treat as exec args
                exec_args = args[i..].to_vec();
                break;
            }
        }
        i += 1;
    }

    // Load from config file if specified
    if let Some(config_path) = config_file {
        let contents = std::fs::read_to_string(&config_path)
            .map_err(|e| JailerError::Config(format!("failed to read config: {e}")))?;
        let mut config: JailConfig = serde_json::from_str(&contents)
            .map_err(|e| JailerError::Config(format!("failed to parse config: {e}")))?;

        // CLI args override config file
        if !exec_args.is_empty() {
            config.exec_args = exec_args;
        }

        return Ok(config);
    }

    // Build config from CLI args
    let id = id.ok_or_else(|| JailerError::Config("--id is required".into()))?;
    let exec_path = exec_path.ok_or_else(|| JailerError::Config("--exec is required".into()))?;
    let jail_root =
        jail_root.ok_or_else(|| JailerError::Config("--jail-root is required".into()))?;

    let mut config = JailConfig::new(id, exec_path, jail_root);

    if let (Some(u), Some(g)) = (uid, gid) {
        config = config.with_uid_gid(u, g);
    }

    if let Some(m) = memory {
        config = config.with_memory_limit(m);
    }

    if let Some(c) = cpu {
        config = config.with_cpu_limit(c);
    }

    if !exec_args.is_empty() {
        config = config.with_args(exec_args);
    }

    Ok(config)
}

/// Parse a size string like "512M" or "1G" into bytes.
#[cfg(target_os = "linux")]
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num, suffix) = if s.ends_with(|c: char| c.is_ascii_alphabetic()) {
        let (n, suf) = s.split_at(s.len() - 1);
        (n, suf)
    } else {
        (s, "")
    };

    let base: u64 = num.parse().ok()?;
    let multiplier = match suffix.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        _ => return None,
    };

    Some(base * multiplier)
}

#[cfg(target_os = "linux")]
fn print_help() {
    eprintln!(
        r#"bencher-jailer - Security isolation for VMM processes

USAGE:
    bencher-jailer [OPTIONS] --id <ID> --exec <PATH> --jail-root <PATH> [-- ARGS...]
    bencher-jailer --config <PATH> [-- ARGS...]

OPTIONS:
    -c, --config <PATH>     Load configuration from JSON file
    --id <ID>               Unique identifier for this jail
    --exec <PATH>           Path to executable to run
    --jail-root <PATH>      Root directory for the jail
    --uid <UID>             User ID to run as (default: 65534)
    --gid <GID>             Group ID to run as (default: 65534)
    --memory <SIZE>         Memory limit (e.g., 512M, 1G)
    --cpu <FLOAT>           CPU limit as fraction (e.g., 0.5, 1.0, 2.0)
    -h, --help              Print this help message

ARGS:
    Arguments after -- are passed to the jailed process

EXAMPLES:
    # Run a VMM with 512MB memory and 1 CPU
    bencher-jailer --id bench-123 \
        --exec /usr/bin/bencher-vmm \
        --jail-root /var/lib/bencher/jails/bench-123 \
        --memory 512M \
        --cpu 1.0 \
        -- --kernel /kernel --rootfs /rootfs.squashfs

    # Run from config file
    bencher-jailer --config /etc/bencher/jail.json
"#
    );
}
