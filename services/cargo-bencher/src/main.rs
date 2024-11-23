use std::env;
use std::process::Command;

fn main() {
    // Collect all command-line arguments
    let args: Vec<String> = env::args().skip(2).collect();

    // Run the `bencher` command with the collected arguments
    let status = Command::new("bencher")
        .args(&args)
        .status()
        .expect("Failed to execute `bencher` command");

    // Check if the command executed successfully
    if !status.success() {
        eprintln!("`bencher` command failed with status: {status}");
    }
}
