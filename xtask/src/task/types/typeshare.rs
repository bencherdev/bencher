use std::process::Command;

use crate::parser::TaskTypeshare;

#[derive(Debug)]
pub struct Typeshare {}

impl TryFrom<TaskTypeshare> for Typeshare {
    type Error = anyhow::Error;

    fn try_from(_typeshare: TaskTypeshare) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Typeshare {
    pub fn exec(&self) -> anyhow::Result<()> {
        Command::new("npm")
            .args(["run", "typeshare"])
            .current_dir("./services/console")
            .status()?;

        println!("Saved to: ./services/console/src/types/bencher.ts");

        Ok(())
    }
}
