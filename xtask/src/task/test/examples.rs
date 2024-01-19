use std::process::Command;

use bencher_json::{Jwt, Url, LOCALHOST_BENCHER_API_URL};

use crate::parser::{TaskExample, TaskExamples};

#[derive(Debug)]
pub struct Examples {
    pub url: Url,
    pub token: Jwt,
    pub example: Option<Example>,
}

#[derive(Debug, Clone, Copy)]
pub enum Example {
    RustBench,
}

impl TryFrom<TaskExamples> for Examples {
    type Error = anyhow::Error;

    fn try_from(examples: TaskExamples) -> Result<Self, Self::Error> {
        let TaskExamples {
            url,
            token,
            example,
        } = examples;
        Ok(Self {
            url: url.unwrap_or_else(|| LOCALHOST_BENCHER_API_URL.clone().into()),
            token: token.unwrap_or_else(Jwt::test_token),
            example: example.map(Into::into),
        })
    }
}

impl From<TaskExample> for Example {
    fn from(example: TaskExample) -> Self {
        match example {
            TaskExample::RustBench => Self::RustBench,
        }
    }
}

impl Examples {
    pub fn exec(&self) -> anyhow::Result<()> {
        if let Some(example) = self.example {
            run_example(&self.url, &self.token, example)
        } else {
            for &example in Example::all() {
                run_example(&self.url, &self.token, example)?;
            }
            Ok(())
        }
    }
}

impl Example {
    pub fn all() -> &'static [Self] {
        &[Self::RustBench]
    }

    pub fn require(self) -> anyhow::Result<()> {
        match self {
            Self::RustBench => {
                Command::new("rustup")
                    .args(["install", "nightly"])
                    .status()?;
                Ok(())
            },
        }
    }

    pub fn dir(&self) -> &str {
        match self {
            Self::RustBench => "./examples/rust/bench",
        }
    }

    pub fn cmd(&self) -> &str {
        match self {
            Self::RustBench => "cargo +nightly bench",
        }
    }
}

fn run_example(api_url: &Url, token: &Jwt, example: Example) -> anyhow::Result<()> {
    println!("Running example: {example:?}");

    example.require()?;

    Command::new("bencher")
        .args([
            "run",
            "--host",
            api_url.as_ref(),
            "--token",
            token.as_ref(),
            "--project",
            "the-computer",
            "--branch",
            "master",
            "--testbed",
            "base",
            example.cmd(),
        ])
        .current_dir(example.dir())
        .status()?;

    Ok(())
}
