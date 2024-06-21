use std::process::Command;

use crate::parser::TaskTs;

#[derive(Debug)]
pub struct Ts {}

impl TryFrom<TaskTs> for Ts {
    type Error = anyhow::Error;

    fn try_from(_typeshare: TaskTs) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Ts {
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
