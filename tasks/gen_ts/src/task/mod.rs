use clap::Parser;

use crate::parser::TaskTypeshare;

mod typeshare;

use typeshare::Typeshare;

#[derive(Debug)]
pub struct Task {
    typeshare: Typeshare,
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            typeshare: TaskTypeshare::parse().try_into()?,
        })
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.typeshare.exec()
    }
}
