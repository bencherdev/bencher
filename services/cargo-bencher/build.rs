use std::process::Command;

fn main() {
    Command::new("cargo")
        .args(&[
            "install",
            "--git",
            "https://github.com/bencherdev/bencher",
            "--branch",
            "main",
            "--locked",
            "--force",
            "bencher_cli",
        ])
        .status()
        .expect("Failed to install bencher CLI");
}
