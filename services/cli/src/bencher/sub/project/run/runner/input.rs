use std::io::BufRead;

#[derive(Debug)]
pub struct Input(String);

impl Input {
    pub fn new() -> Self {
        let mut stdin_buf = String::new();
        let stdin = std::io::stdin();
        let mut handle = stdin.lock();
        while let Ok(size) = handle.read_line(&mut stdin_buf) {
            if size == 0 {
                break;
            }
        }
        Self(stdin_buf)
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
