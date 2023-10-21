use std::process::Command;

use crate::parser::CliSwagger;

const SWAGGER_PATH: &str = "./services/console/src/content/api/swagger.json";

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

        output.status.success().then_some(()).ok_or_else(|| {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            println!("{}", String::from_utf8_lossy(&output.stdout));

            anyhow::anyhow!(
                "Failed to generate swagger.json. Exit code: {:?}",
                output.status.code()
            )
        })?;

        println!("Saved to: {SWAGGER_PATH}");
        swagger_spec()?;

        Ok(())
    }
}

pub fn swagger_spec() -> anyhow::Result<bencher_json::JsonSpec> {
    let swagger_spec_str = std::fs::read_to_string(SWAGGER_PATH)?;
    serde_json::from_str(&swagger_spec_str).map_err(Into::into)
}
