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
#[allow(clippy::enum_variant_names)]
pub enum Example {
    RustBench,
    RustCriterion,
    RustIai,
    RustIaiCallgrind,
    RustCustom,
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
            TaskExample::RustCriterion => Self::RustCriterion,
            TaskExample::RustIai => Self::RustIai,
            TaskExample::RustIaiCallgrind => Self::RustIaiCallgrind,
            TaskExample::RustCustom => Self::RustCustom,
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
        &[
            Self::RustBench,
            Self::RustCriterion,
            #[cfg(target_os = "linux")]
            Self::RustIai,
            #[cfg(target_os = "linux")]
            Self::RustIaiCallgrind,
            Self::RustCustom,
        ]
    }

    pub fn require(self) -> anyhow::Result<()> {
        match self {
            Self::RustBench => {
                // Skip self-update on Windows
                // https://github.com/rust-lang/rustup/issues/3709
                let status = Command::new("rustup")
                    .args(["install", "nightly", "--no-self-update"])
                    .status()?;
                assert!(status.success(), "{status}");
                Ok(())
            },
            Self::RustCriterion | Self::RustCustom => Ok(()),
            Self::RustIai => {
                let status = Command::new("sudo")
                    .args(["apt", "install", "valgrind", "-y"])
                    .status()?;
                assert!(status.success(), "{status}");
                Ok(())
            },
            Self::RustIaiCallgrind => {
                let status = Command::new("sudo")
                    .args(["apt", "install", "valgrind", "-y"])
                    .status()?;
                assert!(status.success(), "{status}");
                let status = Command::new("cargo")
                    .args([
                        "install",
                        "iai-callgrind-runner",
                        "--version",
                        "0.10.2",
                        "--locked",
                        "--force",
                    ])
                    .status()?;
                assert!(status.success(), "{status}");
                Ok(())
            },
        }
    }

    pub fn dir(&self) -> &str {
        match self {
            Self::RustBench => "./examples/rust/bench",
            Self::RustCriterion => "./examples/rust/criterion",
            Self::RustIai => "./examples/rust/iai",
            Self::RustIaiCallgrind => "./examples/rust/iai_callgrind",
            Self::RustCustom => "./examples/rust/custom",
        }
    }

    pub fn args(&self) -> Option<Vec<&str>> {
        match self {
            Self::RustBench | Self::RustCriterion | Self::RustIai | Self::RustIaiCallgrind => None,
            Self::RustCustom => Some(vec!["--file", "results.json"]),
        }
    }

    pub fn cmd(&self) -> &str {
        match self {
            Self::RustBench => "cargo +nightly bench",
            Self::RustCriterion | Self::RustIai | Self::RustIaiCallgrind | Self::RustCustom => {
                "cargo bench"
            },
        }
    }
}

fn run_example(api_url: &Url, token: &Jwt, example: Example) -> anyhow::Result<()> {
    println!("Running example: {example:?}");

    example.require()?;

    let mut args = vec![
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
    ];
    if let Some(example_args) = example.args() {
        args.extend(example_args);
    }
    args.push(example.cmd());

    let status = Command::new("bencher")
        .args(&args)
        .current_dir(example.dir())
        .status()?;
    assert!(status.success(), "{status}");

    Ok(())
}
