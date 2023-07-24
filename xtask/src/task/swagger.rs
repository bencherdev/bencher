use std::process::Command;

use camino::Utf8PathBuf;

use crate::parser::CliSwagger;

#[derive(Debug)]
pub struct Swagger {
    path: Option<Utf8PathBuf>,
}

impl TryFrom<CliSwagger> for Swagger {
    type Error = anyhow::Error;

    fn try_from(swagger: CliSwagger) -> Result<Self, Self::Error> {
        let CliSwagger { path } = swagger;
        Ok(Self {
            path: path.map(Utf8PathBuf::from),
        })
    }
}

impl Swagger {
    pub async fn exec(&self) -> anyhow::Result<()> {
        Command::new("cargo")
            .args(["run", "--bin", "swagger", "--features", "swagger"])
            .current_dir("./services/api")
            .output()?;

        let file_path = self.path.clone().unwrap_or_else(|| {
            Utf8PathBuf::from("./services/ui/src/components/docs/api/swagger.json")
        });
        let output_file = std::fs::read_to_string(file_path)?;
        println!("{output_file}");

        Ok(())
    }
}
