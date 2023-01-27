use std::io::BufRead;

use super::{output::ExitStatus, Output};

#[derive(Debug)]
pub struct Pipe(Output);

impl Pipe {
    pub fn new() -> Option<Self> {
        let mut stdin = String::new();
        let mut stdin_handle = std::io::stdin().lock();
        while let Ok(size) = stdin_handle.read_line(&mut stdin) {
            if size == 0 {
                break;
            }
        }

        let mut stderr = String::new();
        let mut stderr_handle = std::io::stderr().lock();
        while let Ok(size) = stderr_handle.read_line(&mut stderr) {
            if size == 0 {
                break;
            }
        }

        if stdin.is_empty() && stderr.is_empty() {
            None
        } else {
            Some(Self(Output {
                status: ExitStatus::default(),
                stdout: stdin,
                stderr,
            }))
        }
    }

    pub fn output(&self) -> Output {
        self.0.clone()
    }
}
