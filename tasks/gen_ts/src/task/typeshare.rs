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
    #[allow(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        let status = Command::new("npm")
            .args(["run", "typeshare"])
            .current_dir("./services/console")
            .status()?;
        assert!(status.success(), "{status}");

        println!("Saved to: ./services/console/src/types/bencher.ts");

        Ok(())
    }
}
