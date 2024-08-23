use std::process::Command;

fn main() {
    install();
}

fn install() {
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

#[cfg(test)]
mod tests {
    use super::install;

    #[test]
    fn test_install() {
        install();
    }
}
