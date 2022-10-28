use std::io::BufRead;

use crate::CliError;

#[derive(Debug)]
pub struct Input(String);

impl Input {
    pub fn new() -> Result<Self, CliError> {
        let mut stdin_buf = String::new();
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        while let Ok(size) = handle.read_line(&mut stdin_buf) {
            if size == 0 {
                break;
            }
        }
        Ok(Self(stdin_buf))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl ToString for Input {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}
