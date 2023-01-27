use std::io::BufRead;

use super::Output;

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
