use std::{
    fmt,
    io::{BufRead as _, IsTerminal as _},
};

use super::Output;

#[derive(Debug, Clone)]
pub struct Pipe(Output);

impl Pipe {
    pub fn new() -> Option<Self> {
        // If stdin is an interactive TTY, nothing is being piped — bail
        // immediately rather than blocking on `read_line` forever.
        if std::io::stdin().is_terminal() {
            return None;
        }

        let mut stdin = String::new();
        let mut stdin_handle = std::io::stdin().lock();
        while let Ok(size) = stdin_handle.read_line(&mut stdin) {
            if size == 0 {
                break;
            }
        }

        if stdin.is_empty() {
            None
        } else {
            Some(Self(Output {
                stdout: stdin,
                ..Default::default()
            }))
        }
    }

    pub fn output(&self) -> Output {
        self.0.clone()
    }
}

impl fmt::Display for Pipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
