use std::process::Command;

use crate::parser::TaskTypes;

#[derive(Debug)]
pub struct Types {}

impl TryFrom<TaskTypes> for Types {
    type Error = anyhow::Error;

    fn try_from(_types: TaskTypes) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Types {
    pub fn exec(&self) -> anyhow::Result<()> {
        let status = Command::new("cargo").args(["gen-swagger"]).status()?;
        assert!(status.success(), "{status}");

        let status = Command::new("cargo").args(["gen-ts"]).status()?;
        assert!(status.success(), "{status}");

        Ok(())
    }
}
