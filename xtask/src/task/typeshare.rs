use std::process::Command;

use crate::parser::CliTypeshare;

#[derive(Debug)]
pub struct Typeshare {}

impl TryFrom<CliTypeshare> for Typeshare {
    type Error = anyhow::Error;

    fn try_from(_typeshare: CliTypeshare) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Typeshare {
    pub fn exec(&self) -> anyhow::Result<()> {
        let output = Command::new("npm")
            .args(["run", "typeshare"])
            .current_dir("./services/console")
            .output()?;
        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        output.status.success().then_some(()).ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to generate typeshare. Exit code: {:?}",
                output.status.code()
            )
        })?;

        println!("Saved to: ./services/console/src/types/bencher.ts");

        Ok(())
    }
}
