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
    pub async fn exec(&self) -> anyhow::Result<()> {
        let output = Command::new("cargo")
            .args(["run", "--bin", "swagger", "--features", "swagger"])
            .current_dir("./services/api")
            .output()?;
        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        println!("Saved to: ./services/ui/src/components/docs/api/swagger.json");

        Ok(())
    }
}
