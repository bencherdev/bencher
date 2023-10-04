use std::process::Command;

use crate::parser::CliSwagger;

#[derive(Debug)]
pub struct Swagger {}

impl TryFrom<CliSwagger> for Swagger {
    type Error = anyhow::Error;

    fn try_from(_swagger: CliSwagger) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Swagger {
    pub fn exec(&self) -> anyhow::Result<()> {
        let output = Command::new("cargo")
            .args(["run", "--bin", "swagger", "--features", "swagger"])
            .current_dir("./services/api")
            .output()?;
        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        output.status.success().then_some(()).ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to generate swagger.json. Exit code: {:?}",
                output.status.code()
            )
        })?;

        println!("Saved to: ./services/console/src/content/api/swagger.json");

        Ok(())
    }
}
